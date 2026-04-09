//! LLM 统一协议类型。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// URL 引用。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UrlCitation {
    pub url: String,
    pub title: String,
    pub content: String,
    pub start_index: usize,
    pub end_index: usize,
}

/// 注解。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    #[serde(rename = "type")]
    pub annotation_type: String,
    #[serde(default)]
    pub url_citation: Option<UrlCitation>,
}

/// 文本内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub text: String,
    #[serde(default)]
    pub cache_control: Option<Value>,
}

/// 图片内容。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageContent {
    #[serde(rename = "type")]
    pub content_type: String,
    pub image_url: BTreeMap<String, String>,
    #[serde(default)]
    pub media_type: Option<String>,
}

/// 统一消息内容。
pub type MessageContent = Value;

/// Tool call 函数体。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Tool call。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: UnifiedToolCallFunction,
}

/// 统一消息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedMessage {
    pub role: String,
    pub content: Value,
    #[serde(default)]
    pub tool_calls: Option<Vec<UnifiedToolCall>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub cache_control: Option<Value>,
    #[serde(default)]
    pub thinking: Option<Value>,
}

/// 统一工具定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: Value,
}

/// 思考等级。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThinkLevel {
    None,
    Low,
    Medium,
    High,
}

/// 统一聊天请求。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedChatRequest {
    pub messages: Vec<UnifiedMessage>,
    pub model: String,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub tools: Option<Vec<UnifiedTool>>,
    #[serde(default)]
    pub tool_choice: Option<Value>,
    #[serde(default)]
    pub reasoning: Option<Value>,
}

/// usage 信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// 统一聊天响应。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedChatResponse {
    pub id: String,
    pub model: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub tool_calls: Option<Vec<UnifiedToolCall>>,
    #[serde(default)]
    pub annotations: Option<Vec<Annotation>>,
}

/// 流式响应增量。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    #[serde(default)]
    pub choices: Option<Vec<Value>>,
}

/// Provider 定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmProvider {
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub transformer: Option<Value>,
}

/// 路由信息。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelRoute {
    pub provider: String,
    pub model: String,
    pub full_model: String,
}

/// 请求路由结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestRouteInfo {
    pub provider: LlmProvider,
    pub original_model: String,
    pub target_model: String,
}

/// 配置中的 Provider。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigProvider {
    pub name: String,
    pub api_base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub transformer: Option<Value>,
    #[serde(default)]
    pub tokenizer: Option<Value>,
}
