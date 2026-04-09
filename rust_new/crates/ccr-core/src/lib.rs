//! Core 层入口。
//!
//! 该 crate 在 Phase 3 中从骨架升级为可执行版本，
//! 当前聚焦：
//! - Provider 注册与模型路由
//! - Router 决策
//! - Tokenizer/Transformer 串联

mod error;
mod provider_registry;
mod router;
mod service;
mod types;

pub use error::CoreError;
pub use provider_registry::ProviderRegistry;
pub use router::decide_model;
pub use service::{CoreService, ResponseRewriter};
pub use types::{
    PreparedRequest, RouteDecision, RouterConfig, RouterFallbackConfig, RouterScenarioType,
    UsageSnapshot,
};
