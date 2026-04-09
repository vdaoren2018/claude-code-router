//! Tokenizer 层错误定义。

use thiserror::Error;

/// Tokenizer 服务统一错误。
#[derive(Debug, Error)]
pub enum TokenizerError {
    #[error("Tokenizer 不存在: {0}")]
    NotFound(String),

    #[error("Tokenizer 初始化失败: {name} -> {message}")]
    InitFailed { name: String, message: String },

    #[error("Tokenizer 调用失败: {name} -> {message}")]
    Failed { name: String, message: String },

    #[error("配置非法: {0}")]
    InvalidConfig(String),

    #[error("序列化失败: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("HTTP 调用失败: {0}")]
    Http(#[from] reqwest::Error),
}
