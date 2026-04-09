//! Server 层实现（Phase 4）。
//!
//! 当前范围：
//! - 提供基础 HTTP 路由。
//! - 接入 ccr-core 的路由/统计/转换能力。
//! - 支持 `/v1/messages` 非流式与流式转发。

mod error;
mod handlers;
mod routes;
mod sse;
mod state;

use axum::Router;
use ccr_config::{ConfigOptions, ConfigService};
use ccr_core::CoreService;
use tokio::net::TcpListener;

pub use error::ServerError;
use routes::build_router;
use state::AppState;

/// 服务状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerState {
    /// 初始状态。
    Init,
    /// 运行中。
    Running,
    /// 已停止。
    Stopped,
}

/// HTTP 服务对象。
pub struct Server {
    /// 当前状态。
    pub state: ServerState,
    /// 监听地址。
    host: String,
    /// 监听端口。
    port: u16,
    /// 共享应用状态。
    app_state: AppState,
}

impl Server {
    /// 从配置服务创建 Server。
    pub fn from_config_service(config_service: ConfigService) -> Self {
        let host = config_service.get_or("HOST", "127.0.0.1".to_string());
        let port = config_service.get_or("PORT", 3000_u16);

        let core_service = CoreService::new(config_service);
        Self::new(core_service, host, port)
    }

    /// 从 ConfigOptions 快速创建 Server。
    pub fn from_config_options(options: ConfigOptions) -> Result<Self, ServerError> {
        let config_service = ConfigService::new(options)
            .map_err(|err| ServerError::Internal(format!("初始化配置失败: {err}")))?;
        Ok(Self::from_config_service(config_service))
    }

    /// 使用已初始化 CoreService 创建 Server。
    pub fn new(core_service: CoreService, host: impl Into<String>, port: u16) -> Self {
        Self {
            state: ServerState::Init,
            host: host.into(),
            port,
            app_state: AppState::new(core_service),
        }
    }

    /// 返回监听地址字符串。
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    /// 构建 axum 应用（用于测试或嵌入式运行）。
    pub fn app(&self) -> Router {
        build_router(self.app_state.clone())
    }

    /// 启动服务（阻塞直到退出）。
    pub async fn start(&mut self) -> Result<(), ServerError> {
        let listener = TcpListener::bind(self.address()).await.map_err(|err| {
            ServerError::Internal(format!("绑定监听地址失败({}): {err}", self.address()))
        })?;

        self.state = ServerState::Running;

        axum::serve(listener, self.app())
            .await
            .map_err(|err| ServerError::Internal(format!("服务运行失败: {err}")))?;

        self.state = ServerState::Stopped;
        Ok(())
    }

    /// 主动标记停止（供上层生命周期管理使用）。
    pub fn stop(&mut self) {
        self.state = ServerState::Stopped;
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router,
        body::{Body, to_bytes},
        http::{Request, StatusCode, header},
        response::IntoResponse,
        routing::post,
    };
    use ccr_config::{ConfigOptions, ConfigService};
    use serde_json::{Value, json};
    use tokio::{net::TcpListener, sync::oneshot};
    use tower::util::ServiceExt;

    use super::Server;

    /// 构造内存配置。
    fn config_service_with(value: Value) -> ConfigService {
        ConfigService::new(ConfigOptions {
            use_json_file: false,
            use_env_file: false,
            use_environment_variables: false,
            initial_config: Some(value),
            ..ConfigOptions::default()
        })
        .expect("create config service")
    }

    /// 读取 JSON 响应体。
    async fn read_json(response: axum::response::Response) -> Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body bytes");
        serde_json::from_slice(&bytes).expect("parse json")
    }

    /// 读取文本响应体。
    async fn read_text(response: axum::response::Response) -> String {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body bytes");
        String::from_utf8(bytes.to_vec()).expect("utf8 text")
    }

    /// 启动 mock provider。
    async fn start_mock_provider() -> (String, oneshot::Sender<()>) {
        let app = Router::new().route(
            "/v1/chat/completions",
            post(|Json(payload): Json<Value>| async move {
                if payload
                    .get("stream")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
                {
                    let sse_text = "data: {\"id\":\"chunk-1\",\"model\":\"gpt-5\",\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n";
                    return (
                        [(header::CONTENT_TYPE, "text/event-stream; charset=utf-8")],
                        sse_text,
                    )
                        .into_response();
                }

                Json(json!({
                    "id": "mock-response",
                    "model": payload.get("model").cloned().unwrap_or_else(|| json!("unknown")),
                    "content": "ok"
                }))
                .into_response()
            }),
        );

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock provider");
        let address = listener.local_addr().expect("local addr");

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async {
                let _ = shutdown_rx.await;
            });
            let _ = server.await;
        });

        (format!("http://{address}/v1/chat/completions"), shutdown_tx)
    }

    #[tokio::test]
    async fn test_api_config_endpoint() {
        let server = Server::from_config_service(config_service_with(json!({
            "providers": [],
            "Router": {"default": "openai,gpt-5"}
        })));

        let response = server
            .app()
            .oneshot(
                Request::builder()
                    .uri("/api/config")
                    .method("GET")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("execute request");

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body["Router"]["default"], json!("openai,gpt-5"));
    }

    #[tokio::test]
    async fn test_count_tokens_endpoint() {
        let server = Server::from_config_service(config_service_with(json!({
            "providers": []
        })));

        let response = server
            .app()
            .oneshot(
                Request::builder()
                    .uri("/v1/messages/count_tokens")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "messages": [{"role": "user", "content": "hello rust"}],
                            "system": "system prompt"
                        })
                        .to_string(),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("execute request");

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert!(body["token_count"].as_u64().unwrap_or_default() > 0);
    }

    #[tokio::test]
    async fn test_route_preview_web_search() {
        let server = Server::from_config_service(config_service_with(json!({
            "providers": [
                {
                    "name": "openai",
                    "api_base_url": "https://api.openai.com/v1/chat/completions",
                    "api_key": "sk-openai",
                    "models": ["gpt-5", "gpt-5-search"]
                }
            ],
            "Router": {
                "default": "openai,gpt-5",
                "webSearch": "openai,gpt-5-search",
                "think": "openai,gpt-5"
            }
        })));

        let response = server
            .app()
            .oneshot(
                Request::builder()
                    .uri("/api/route/preview")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "gpt-5",
                            "messages": [{"role": "user", "content": "search news"}],
                            "tools": [{"type": "web_search_preview"}],
                            "thinking": {"type": "enabled"}
                        })
                        .to_string(),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("execute request");

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body["scenario_type"], json!("webSearch"));
        assert_eq!(body["model"], json!("openai,gpt-5-search"));
    }

    #[tokio::test]
    async fn test_v1_messages_forward_to_provider() {
        let (provider_url, shutdown_tx) = start_mock_provider().await;

        let server = Server::from_config_service(config_service_with(json!({
            "providers": [
                {
                    "name": "openai",
                    "api_base_url": provider_url,
                    "api_key": "sk-openai",
                    "models": ["gpt-5"]
                }
            ],
            "Router": {
                "default": "openai,gpt-5"
            }
        })));

        let response = server
            .app()
            .oneshot(
                Request::builder()
                    .uri("/v1/messages")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "openai,gpt-5",
                            "messages": [{"role": "user", "content": "hello"}],
                            "stream": false
                        })
                        .to_string(),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("execute request");

        let _ = shutdown_tx.send(());

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json(response).await;
        assert_eq!(body["id"], json!("mock-response"));
        assert_eq!(body["model"], json!("gpt-5"));
    }

    #[tokio::test]
    async fn test_v1_messages_stream_forward_to_provider() {
        let (provider_url, shutdown_tx) = start_mock_provider().await;

        let server = Server::from_config_service(config_service_with(json!({
            "providers": [
                {
                    "name": "openai",
                    "api_base_url": provider_url,
                    "api_key": "sk-openai",
                    "models": ["gpt-5"]
                }
            ],
            "Router": {
                "default": "openai,gpt-5"
            }
        })));

        let response = server
            .app()
            .oneshot(
                Request::builder()
                    .uri("/v1/messages")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "model": "openai,gpt-5",
                            "messages": [{"role": "user", "content": "hello stream"}],
                            "stream": true
                        })
                        .to_string(),
                    ))
                    .expect("build request"),
            )
            .await
            .expect("execute request");

        let _ = shutdown_tx.send(());

        assert_eq!(response.status(), StatusCode::OK);
        let content_type = response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string();
        assert!(content_type.starts_with("text/event-stream"));

        let text = read_text(response).await;
        assert!(text.contains("data: {"));
        assert!(text.contains("\"chunk-1\""));
        assert!(text.contains("data: [DONE]"));
    }
}
