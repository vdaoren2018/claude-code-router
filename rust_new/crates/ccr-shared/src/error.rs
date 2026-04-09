//! 共享错误定义。

use std::path::PathBuf;

use thiserror::Error;

/// 共享层统一错误。
#[derive(Debug, Error)]
pub enum SharedError {
    /// IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// JSON5 解析错误。
    #[error("JSON5 解析失败: {0}")]
    Json5(String),

    /// JSON 序列化/反序列化错误。
    #[error("JSON 错误: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP 请求错误。
    #[error("HTTP 请求失败: {0}")]
    Http(#[from] reqwest::Error),

    /// ZIP 解析错误。
    #[error("ZIP 处理失败: {0}")]
    Zip(#[from] zip::result::ZipError),

    /// 预设名非法。
    #[error("非法预设名: {0}")]
    InvalidPresetName(String),

    /// 预设已存在。
    #[error("预设目录已存在: {0}")]
    PresetAlreadyExists(String),

    /// 路径穿越风险。
    #[error("检测到路径穿越风险: {0}")]
    PathTraversal(String),

    /// 文件缺失。
    #[error("文件不存在: {0}")]
    MissingFile(PathBuf),

    /// 业务校验失败。
    #[error("校验失败: {0}")]
    Validation(String),
}

/// 小工具：将 json5 错误转为统一错误。
pub fn json5_err(err: impl std::fmt::Display) -> SharedError {
    SharedError::Json5(err.to_string())
}
