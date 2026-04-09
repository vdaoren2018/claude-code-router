//! Core 层领域类型。
//!
//! 该模块只承载“路由与执行过程”相关的数据结构，
//! 不放行为逻辑，方便在 service/router 中复用。

use ccr_protocol::{RequestRouteInfo, UnifiedChatRequest};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 路由场景类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RouterScenarioType {
    /// 默认场景。
    Default,
    /// Claude Haiku 自动下沉到后台场景。
    Background,
    /// 思考增强场景。
    Think,
    /// 长上下文场景。
    LongContext,
    /// Web 搜索工具优先场景。
    WebSearch,
}

impl RouterScenarioType {
    /// 返回场景字符串。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Background => "background",
            Self::Think => "think",
            Self::LongContext => "longContext",
            Self::WebSearch => "webSearch",
        }
    }
}

/// 最近一次会话 usage 快照。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsageSnapshot {
    /// 历史输入 token。
    #[serde(default)]
    pub input_tokens: u64,
    /// 历史输出 token。
    #[serde(default)]
    pub output_tokens: u64,
    /// 历史总 token。
    #[serde(default)]
    pub total_tokens: u64,
}

/// Router 配置（对应 TS 中 `Router`）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterConfig {
    /// 默认模型。
    #[serde(default)]
    pub default: Option<String>,
    /// 后台模型。
    #[serde(default)]
    pub background: Option<String>,
    /// 思考模型。
    #[serde(default)]
    pub think: Option<String>,
    /// 长上下文模型。
    #[serde(default, rename = "longContext")]
    pub long_context: Option<String>,
    /// 长上下文阈值。
    #[serde(default, rename = "longContextThreshold")]
    pub long_context_threshold: Option<u64>,
    /// Web 搜索模型。
    #[serde(default, rename = "webSearch")]
    pub web_search: Option<String>,
}

/// 路由 fallback 配置占位。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouterFallbackConfig {
    /// default 场景 fallback 列表。
    #[serde(default)]
    pub default: Vec<String>,
    /// background 场景 fallback 列表。
    #[serde(default)]
    pub background: Vec<String>,
    /// think 场景 fallback 列表。
    #[serde(default)]
    pub think: Vec<String>,
    /// longContext 场景 fallback 列表。
    #[serde(default, rename = "longContext")]
    pub long_context: Vec<String>,
    /// webSearch 场景 fallback 列表。
    #[serde(default, rename = "webSearch")]
    pub web_search: Vec<String>,
}

/// 路由决策结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteDecision {
    /// 最终选中的模型（可能是 `provider,model`）。
    pub model: String,
    /// 场景类型。
    pub scenario_type: RouterScenarioType,
}

/// Core 预处理输出。
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedRequest {
    /// 路由决策。
    pub route_decision: RouteDecision,
    /// Provider 路由解析结果。
    pub route_info: RequestRouteInfo,
    /// 本次请求 token 数。
    pub token_count: u64,
    /// 实际使用的 tokenizer 名称。
    pub tokenizer_used: String,
    /// 本次 token 统计是否命中缓存。
    pub tokenizer_cached: bool,
    /// 本次执行的 transformer 链（按执行顺序）。
    pub transformer_chain: Vec<String>,
    /// 统一协议请求（模型已替换为 provider 目标模型名）。
    pub unified_request: UnifiedChatRequest,
    /// 发往 provider 的 payload（已经过 transformer + auth）。
    pub provider_payload: Value,
}
