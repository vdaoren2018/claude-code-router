//! 简化版本地 tokenizer。
//!
//! 说明：
//! - 该实现用于 Phase 2 的可执行版本。
//! - 目标是提供稳定、可复用、可 fallback 的基线计数能力。
//! - 后续可替换成真实 tiktoken/hf 实现而不影响上层接口。

use std::sync::LazyLock;

use async_trait::async_trait;
use ccr_protocol::{TokenizeRequest, Tokenizer};
use regex::Regex;
use serde_json::Value;

/// 词元拆分正则。
static TOKEN_SPLIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\s\p{P}]+").expect("token split regex 构建失败"));

/// 简化本地 tokenizer。
pub struct SimpleTokenizer {
    /// 编码名（用于标识，不参与真实编码）。
    encoding: String,
    /// 是否已初始化。
    initialized: bool,
}

impl SimpleTokenizer {
    /// 创建实例。
    pub fn new(encoding: impl Into<String>) -> Self {
        Self {
            encoding: encoding.into(),
            initialized: false,
        }
    }

    /// 统计文本 token。
    ///
    /// 策略：
    /// - 基于标点与空白分词。
    /// - 对中文等场景额外采用字符估算兜底。
    fn count_text_tokens(text: &str) -> u64 {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return 0;
        }

        let split_count = TOKEN_SPLIT_RE
            .split(trimmed)
            .filter(|part| !part.is_empty())
            .count() as u64;

        // 对非空白语言的兜底估算，避免全部聚合为 1 token。
        let char_estimate = (trimmed.chars().count() as u64).div_ceil(4);

        split_count.max(char_estimate).max(1)
    }

    /// 递归统计 JSON 中的字符串 token。
    fn count_value_tokens(value: &Value) -> u64 {
        match value {
            Value::Null | Value::Bool(_) | Value::Number(_) => 0,
            Value::String(text) => Self::count_text_tokens(text),
            Value::Array(list) => list.iter().map(Self::count_value_tokens).sum(),
            Value::Object(map) => map
                .iter()
                .map(|(key, value)| Self::count_text_tokens(key) + Self::count_value_tokens(value))
                .sum(),
        }
    }
}

#[async_trait]
impl Tokenizer for SimpleTokenizer {
    fn tokenizer_type(&self) -> &str {
        "tiktoken"
    }

    fn name(&self) -> &str {
        &self.encoding
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.initialized = true;
        Ok(())
    }

    async fn count_tokens(&self, request: &TokenizeRequest) -> Result<u64, String> {
        let mut total = 0u64;

        for message in &request.messages {
            total += Self::count_value_tokens(message);
        }

        if let Some(system) = &request.system {
            total += Self::count_value_tokens(system);
        }

        if let Some(tools) = &request.tools {
            for tool in tools {
                total += Self::count_value_tokens(tool);
            }
        }

        Ok(total)
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn dispose(&mut self) {
        self.initialized = false;
    }
}
