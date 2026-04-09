//! 配置层错误定义。

use thiserror::Error;

/// 配置服务统一错误。
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON5 解析失败: {0}")]
    Json5(String),

    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    #[error("配置无效: {0}")]
    InvalidConfig(String),
}
