//! Transformer 协议定义。

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::llm::{LlmProvider, UnifiedChatRequest};

/// Transformer 选项。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TransformerOptions {
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Transformer 上下文。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TransformerContext {
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Transformer trait。
#[async_trait]
pub trait Transformer: Send + Sync {
    /// 名称。
    fn name(&self) -> &str;

    /// 可选 endpoint。
    fn endpoint(&self) -> Option<&str> {
        None
    }

    /// 入站请求转换。
    async fn transform_request_in(
        &self,
        request: &UnifiedChatRequest,
        provider: &LlmProvider,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        let _ = provider;
        serde_json::to_value(request).map_err(|e| e.to_string())
    }

    /// 出站请求转换到统一格式。
    async fn transform_request_out(
        &self,
        request: &Value,
        _context: &TransformerContext,
    ) -> Result<UnifiedChatRequest, String> {
        serde_json::from_value(request.clone()).map_err(|e| e.to_string())
    }

    /// 响应入站转换。
    async fn transform_response_in(
        &self,
        response: &Value,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        Ok(response.clone())
    }

    /// 响应出站转换。
    async fn transform_response_out(
        &self,
        response: &Value,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        Ok(response.clone())
    }

    /// 鉴权扩展。
    async fn auth(
        &self,
        request: &Value,
        _provider: &LlmProvider,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        Ok(request.clone())
    }
}
