//! 路由注册。
//!
//! 路由拆分独立文件，避免 `lib.rs` 膨胀，便于后续继续扩展流式接口。

use axum::{
    Router,
    routing::{get, post},
};

use crate::handlers::{
    api_config, api_health, api_providers, count_tokens, post_messages, root, route_preview,
};
use crate::state::AppState;

/// 构建应用 Router。
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/api/health", get(api_health))
        .route("/api/config", get(api_config))
        .route("/api/providers", get(api_providers))
        .route("/api/route/preview", post(route_preview))
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/messages", post(post_messages))
        .with_state(state)
}
