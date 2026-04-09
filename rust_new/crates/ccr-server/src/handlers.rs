//! HTTP 处理函数。
//!
//! 当前阶段（Phase 4）目标：
//! - 提供基础 `/api/*` 能力。
//! - 提供 `/v1/messages` 非流式与流式能力。
//! - 提供 `/v1/messages/count_tokens` 本地统计。

use async_stream::try_stream;
use axum::{
    Json,
    body::{Body, Bytes},
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use futures_util::{StreamExt, stream::Stream};
use serde::Serialize;
use serde_json::{Value, json};

use crate::error::ServerError;
use crate::sse::{SseLineBuffer, rewrite_sse_line};
use crate::state::AppState;

/// 根路径响应。
#[derive(Debug, Clone, Serialize)]
pub struct RootResponse {
    /// 服务名。
    pub message: &'static str,
    /// 当前 Rust 服务阶段标识。
    pub stage: &'static str,
}

/// 健康检查响应。
#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    /// 固定 ok。
    pub status: &'static str,
}

/// 计数接口响应。
#[derive(Debug, Clone, Serialize)]
pub struct CountTokensResponse {
    /// token 数。
    pub token_count: u64,
    /// tokenizer 名称。
    pub tokenizer_used: String,
    /// 是否命中缓存。
    pub cached: bool,
}

/// 路由预览响应。
#[derive(Debug, Clone, Serialize)]
pub struct RoutePreviewResponse {
    /// 场景类型。
    pub scenario_type: String,
    /// 最终模型。
    pub model: String,
    /// provider 名称。
    pub provider: String,
    /// provider 目标模型。
    pub provider_model: String,
    /// token 数。
    pub token_count: u64,
    /// tokenizer 名称。
    pub tokenizer_used: String,
    /// transformer 执行链。
    pub transformer_chain: Vec<String>,
}

/// `GET /`。
pub async fn root() -> Json<RootResponse> {
    Json(RootResponse {
        message: "CCR Rust Server",
        stage: "phase4",
    })
}

/// `GET /api/health`。
pub async fn api_health() -> Json<HealthResponse> {
    Json(HealthResponse { status: "ok" })
}

/// `GET /api/config`。
pub async fn api_config(State(state): State<AppState>) -> Json<Value> {
    let core_service = state.core_service.lock().await;
    Json(core_service.config_snapshot())
}

/// `GET /api/providers`。
pub async fn api_providers(State(state): State<AppState>) -> Json<Value> {
    let core_service = state.core_service.lock().await;
    Json(json!(core_service.providers()))
}

/// `POST /v1/messages/count_tokens`。
pub async fn count_tokens(
    State(state): State<AppState>,
    Json(request_body): Json<Value>,
) -> Result<Json<CountTokensResponse>, ServerError> {
    let mut core_service = state.core_service.lock().await;
    let token_result = core_service.count_tokens(&request_body).await?;

    Ok(Json(CountTokensResponse {
        token_count: token_result.token_count,
        tokenizer_used: token_result.tokenizer_used,
        cached: token_result.cached,
    }))
}

/// `POST /api/route/preview`。
///
/// 说明：
/// - 仅做本地路由与转换预处理，不调用 provider。
/// - 便于迁移期间做行为比对。
pub async fn route_preview(
    State(state): State<AppState>,
    Json(mut request_body): Json<Value>,
) -> Result<Json<RoutePreviewResponse>, ServerError> {
    force_stream_flag(&mut request_body, false);

    let mut core_service = state.core_service.lock().await;
    let prepared = core_service
        .prepare_request(&mut request_body, None)
        .await?;

    Ok(Json(RoutePreviewResponse {
        scenario_type: prepared.route_decision.scenario_type.as_str().to_string(),
        model: prepared.route_decision.model,
        provider: prepared.route_info.provider.name,
        provider_model: prepared.route_info.target_model,
        token_count: prepared.token_count,
        tokenizer_used: prepared.tokenizer_used,
        transformer_chain: prepared.transformer_chain,
    }))
}

/// `POST /v1/messages`。
///
/// 当前行为：
/// 1. 支持 `stream=false` 与 `stream=true`。
/// 2. 在转发前执行本地路由、tokenizer 与 transformer 请求链。
/// 3. 对响应执行 transformer 响应重写链。
pub async fn post_messages(
    State(state): State<AppState>,
    Json(mut request_body): Json<Value>,
) -> Result<Response, ServerError> {
    let stream_enabled = request_body
        .get("stream")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    // 锁住 core_service 仅做“准备阶段”，避免持锁穿透整个网络请求生命周期。
    let (prepared, response_rewriter) = {
        let mut core_service = state.core_service.lock().await;
        let prepared = core_service
            .prepare_request(&mut request_body, None)
            .await?;
        let response_rewriter = core_service.build_response_rewriter(&prepared)?;
        (prepared, response_rewriter)
    };

    let response = state
        .http_client
        .post(&prepared.route_info.provider.base_url)
        .bearer_auth(&prepared.route_info.provider.api_key)
        .json(&prepared.provider_payload)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body_text = response.text().await?;
        return Err(ServerError::Upstream {
            status: StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            body: body_text,
        });
    }

    if stream_enabled {
        return stream_provider_response(response, response_rewriter).await;
    }

    // 非流式路径：读取 JSON 后执行响应重写链。
    let body_text = response.text().await?;
    let body_value = parse_json_or_text(&body_text);
    let rewritten_value = response_rewriter.rewrite(&body_value).await?;
    Ok((StatusCode::OK, Json(rewritten_value)).into_response())
}

/// 处理 provider 的 SSE 流式响应。
async fn stream_provider_response(
    response: reqwest::Response,
    response_rewriter: ccr_core::ResponseRewriter,
) -> Result<Response, ServerError> {
    let rewritten_stream = rewrite_sse_stream(response.bytes_stream(), response_rewriter);

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/event-stream; charset=utf-8"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    headers.insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));

    Ok((StatusCode::OK, headers, Body::from_stream(rewritten_stream)).into_response())
}

/// 把上游字节流重写为 SSE 输出流。
fn rewrite_sse_stream<S>(
    upstream_stream: S,
    response_rewriter: ccr_core::ResponseRewriter,
) -> impl Stream<Item = Result<Bytes, std::io::Error>>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin + Send + 'static,
{
    try_stream! {
        let mut line_buffer = SseLineBuffer::new();
        futures_util::pin_mut!(upstream_stream);

        while let Some(chunk_result) = upstream_stream.next().await {
            let chunk = chunk_result.map_err(|err| std::io::Error::other(err.to_string()))?;

            // 逐行处理，保证跨 chunk 也能得到完整 SSE 行。
            let lines = line_buffer.push_chunk(&chunk);
            for line in lines {
                let rewritten_line = rewrite_sse_line(&line, &response_rewriter)
                    .await
                    .map_err(|err| std::io::Error::other(err.to_string()))?;
                yield Bytes::from(format!("{rewritten_line}\n"));
            }
        }

        // 刷出尾部残留行。
        if let Some(last_line) = line_buffer.finish() {
            let rewritten_line = rewrite_sse_line(&last_line, &response_rewriter)
                .await
                .map_err(|err| std::io::Error::other(err.to_string()))?;
            yield Bytes::from(format!("{rewritten_line}\n"));
        }
    }
}

/// 强制设置 stream 开关。
fn force_stream_flag(request_body: &mut Value, enabled: bool) {
    let Some(map) = request_body.as_object_mut() else {
        return;
    };
    map.insert("stream".to_string(), Value::Bool(enabled));
}

/// 解析 JSON 文本；失败时按纯文本返回。
fn parse_json_or_text(body_text: &str) -> Value {
    if body_text.trim().is_empty() {
        return Value::Object(Default::default());
    }

    serde_json::from_str(body_text).unwrap_or_else(|_| {
        json!({
            "raw": body_text,
        })
    })
}
