//! API tokenizer 实现。
//!
//! 目标：
//! - 支持通过 HTTP 端点做 token 统计。
//! - 支持 response_field 路径提取。
//! - 与本地 tokenizer 一起形成 fallback 链。

use async_trait::async_trait;
use ccr_protocol::{ApiRequestFormat, TokenizeRequest, Tokenizer};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::{Value, json};

/// API tokenizer。
pub struct ApiTokenizer {
    /// endpoint。
    url: String,
    /// 可选 API key。
    api_key: Option<String>,
    /// 请求格式。
    request_format: ApiRequestFormat,
    /// 响应字段路径。
    response_field: Option<String>,
    /// 自定义 headers。
    headers: Option<std::collections::BTreeMap<String, String>>,
    /// 初始化状态。
    initialized: bool,
    /// HTTP 客户端。
    client: reqwest::Client,
}

impl ApiTokenizer {
    /// 创建实例。
    pub fn new(
        url: impl Into<String>,
        api_key: Option<String>,
        request_format: Option<ApiRequestFormat>,
        response_field: Option<String>,
        headers: Option<std::collections::BTreeMap<String, String>>,
    ) -> Self {
        Self {
            url: url.into(),
            api_key,
            request_format: request_format.unwrap_or(ApiRequestFormat::Standard),
            response_field,
            headers,
            initialized: false,
            client: reqwest::Client::new(),
        }
    }

    /// 构造请求体。
    fn build_payload(&self, request: &TokenizeRequest) -> Value {
        match self.request_format {
            ApiRequestFormat::Standard => json!({
                "messages": request.messages,
                "system": request.system,
                "tools": request.tools,
            }),
            ApiRequestFormat::Openai => json!({
                "messages": request.messages,
                "system": request.system,
                "tools": request.tools,
            }),
            ApiRequestFormat::Anthropic => json!({
                "messages": request.messages,
                "system": request.system,
                "tools": request.tools,
            }),
            ApiRequestFormat::Custom => json!({
                "payload": {
                    "messages": request.messages,
                    "system": request.system,
                    "tools": request.tools,
                }
            }),
        }
    }

    /// 构造 header。
    fn build_headers(&self) -> Result<HeaderMap, String> {
        let mut map = HeaderMap::new();
        map.insert("content-type", HeaderValue::from_static("application/json"));

        if let Some(api_key) = &self.api_key {
            let bearer = format!("Bearer {api_key}");
            let value = HeaderValue::from_str(&bearer).map_err(|e| e.to_string())?;
            map.insert("authorization", value);
        }

        if let Some(headers) = &self.headers {
            for (key, value) in headers {
                let name = HeaderName::from_bytes(key.as_bytes()).map_err(|e| e.to_string())?;
                let value = HeaderValue::from_str(value).map_err(|e| e.to_string())?;
                map.insert(name, value);
            }
        }

        Ok(map)
    }

    /// 按路径提取 token 字段。
    fn extract_token_count(&self, payload: &Value) -> Option<u64> {
        if let Some(path) = &self.response_field {
            if let Some(value) = get_path_value(payload, path) {
                return value.as_u64();
            }
        }

        payload
            .get("token_count")
            .and_then(Value::as_u64)
            .or_else(|| payload.get("total_tokens").and_then(Value::as_u64))
            .or_else(|| {
                payload
                    .get("usage")
                    .and_then(|usage| usage.get("total_tokens"))
                    .and_then(Value::as_u64)
            })
    }
}

#[async_trait]
impl Tokenizer for ApiTokenizer {
    fn tokenizer_type(&self) -> &str {
        "api"
    }

    fn name(&self) -> &str {
        &self.url
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.initialized = true;
        Ok(())
    }

    async fn count_tokens(&self, request: &TokenizeRequest) -> Result<u64, String> {
        let payload = self.build_payload(request);
        let headers = self.build_headers()?;

        let response = self
            .client
            .post(&self.url)
            .headers(headers)
            .json(&payload)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !response.status().is_success() {
            return Err(format!("API tokenizer 返回错误状态: {}", response.status()));
        }

        let body = response.json::<Value>().await.map_err(|e| e.to_string())?;
        self.extract_token_count(&body)
            .ok_or_else(|| "无法从 API 响应中解析 token 字段".to_string())
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn dispose(&mut self) {
        self.initialized = false;
    }
}

/// 按点路径读取值，例如 `usage.total_tokens`。
fn get_path_value<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = value;

    for part in path.split('.') {
        current = current.get(part)?;
    }

    Some(current)
}
