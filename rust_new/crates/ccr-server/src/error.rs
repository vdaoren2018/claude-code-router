//! Server 层错误定义。
//!
//! 说明：
//! - 该错误类型既用于内部流程控制，也负责映射 HTTP 响应。
//! - 对上游 provider 错误保留状态码与原始文本，便于排查。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

/// 错误响应体。
#[derive(Debug, Clone, Serialize)]
struct ErrorBody {
    /// 业务错误码。
    code: &'static str,
    /// 人类可读错误信息。
    message: String,
}

/// Server 层统一错误。
#[derive(Debug, Error)]
pub enum ServerError {
    /// 请求参数非法。
    #[error("请求非法: {0}")]
    BadRequest(String),

    /// Core 层错误透传。
    #[error(transparent)]
    Core(#[from] ccr_core::CoreError),

    /// HTTP 客户端错误。
    #[error(transparent)]
    Http(#[from] reqwest::Error),

    /// 上游 provider 返回错误状态码。
    #[error("上游 Provider 错误({status}): {body}")]
    Upstream { status: StatusCode, body: String },

    /// 内部错误兜底。
    #[error("内部错误: {0}")]
    Internal(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(message) => {
                error_response(StatusCode::BAD_REQUEST, "bad_request", message)
            }
            Self::Core(err) => {
                error_response(StatusCode::BAD_REQUEST, "core_error", err.to_string())
            }
            Self::Http(err) => error_response(
                StatusCode::BAD_GATEWAY,
                "upstream_transport_error",
                err.to_string(),
            ),
            Self::Upstream { status, body } => error_response(status, "upstream_error", body),
            Self::Internal(message) => {
                error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
            }
        }
    }
}

/// 构造统一错误响应。
fn error_response(status: StatusCode, code: &'static str, message: String) -> Response {
    (status, Json(ErrorBody { code, message })).into_response()
}
