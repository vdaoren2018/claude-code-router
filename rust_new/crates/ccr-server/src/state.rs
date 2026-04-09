//! Server 共享状态。
//!
//! 由于 `CoreService` 的 tokenizer 带内部缓存，
//! 其方法多数需要 `&mut self`，因此这里用 `Mutex` 做串行化访问。

use std::sync::Arc;

use ccr_core::CoreService;
use tokio::sync::Mutex;

/// Axum 共享状态。
#[derive(Clone)]
pub struct AppState {
    /// Core 聚合服务。
    pub core_service: Arc<Mutex<CoreService>>,
    /// 对外 HTTP 请求客户端（用于转发到 provider）。
    pub http_client: reqwest::Client,
}

impl AppState {
    /// 创建共享状态。
    pub fn new(core_service: CoreService) -> Self {
        Self {
            core_service: Arc::new(Mutex::new(core_service)),
            http_client: reqwest::Client::new(),
        }
    }
}
