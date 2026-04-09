//! Core 层错误定义。
//!
//! 设计原则：
//! - 统一收敛 ccr-core 对外错误，便于上层（server/cli）分类处理。
//! - 对下层模块（tokenizer/transform）错误做透明透传，避免丢失上下文。

use thiserror::Error;

/// Core 层统一错误。
#[derive(Debug, Error)]
pub enum CoreError {
    /// Provider 未找到。
    #[error("Provider 不存在: {0}")]
    ProviderNotFound(String),

    /// 模型路由未找到。
    #[error("模型路由不存在: {0}")]
    RouteNotFound(String),

    /// 配置值不合法。
    #[error("配置非法: {0}")]
    InvalidConfig(String),

    /// 请求体不合法或缺失关键字段。
    #[error("请求非法: {0}")]
    RequestInvalid(String),

    /// 缺少模型字段。
    #[error("请求缺少 model 字段")]
    MissingModel,

    /// Tokenizer 子系统错误。
    #[error(transparent)]
    Tokenizer(#[from] ccr_tokenizer::TokenizerError),

    /// Transformer 子系统错误。
    #[error(transparent)]
    Transform(#[from] ccr_transform::TransformError),

    /// JSON 序列化/反序列化错误。
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
}
