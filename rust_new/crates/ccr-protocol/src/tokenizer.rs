//! Tokenizer 协议定义。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Tokenizer 类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenizerType {
    Tiktoken,
    Huggingface,
    Api,
}

/// API 请求格式。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ApiRequestFormat {
    Standard,
    Openai,
    Anthropic,
    Custom,
}

/// Tokenizer 配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenizerConfig {
    pub tokenizer_type: TokenizerType,
    #[serde(default)]
    pub encoding: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub request_format: Option<ApiRequestFormat>,
    #[serde(default)]
    pub response_field: Option<String>,
    #[serde(default)]
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub fallback: Option<TokenizerType>,
}

/// Tokenizer 服务配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TokenizerOptions {
    #[serde(default)]
    pub cache_enabled: Option<bool>,
    #[serde(default)]
    pub cache_size: Option<usize>,
    #[serde(default)]
    pub timeout: Option<u64>,
}

/// Token 统计请求。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenizeRequest {
    pub messages: Vec<serde_json::Value>,
    #[serde(default)]
    pub system: Option<serde_json::Value>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
}

/// Token 统计结果。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TokenizerResult {
    pub token_count: u64,
    pub tokenizer_used: String,
    pub cached: bool,
}

/// Tokenizer 抽象接口。
#[async_trait]
pub trait Tokenizer: Send + Sync {
    fn tokenizer_type(&self) -> &str;
    fn name(&self) -> &str;
    async fn initialize(&mut self) -> Result<(), String>;
    async fn count_tokens(&self, request: &TokenizeRequest) -> Result<u64, String>;
    fn is_initialized(&self) -> bool;
    fn dispose(&mut self);
}

/// Provider 级 tokenizer 配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ProviderTokenizerConfig {
    #[serde(default)]
    pub default: Option<TokenizerConfig>,
    #[serde(default)]
    pub models: Option<std::collections::BTreeMap<String, TokenizerConfig>>,
}
