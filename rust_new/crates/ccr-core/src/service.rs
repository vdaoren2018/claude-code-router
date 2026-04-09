//! Core 服务实现。
//!
//! 该服务负责把配置、路由、tokenizer、transformer 串起来，
//! 生成“可直接发送到 provider”的请求 payload。

use ccr_config::ConfigService;
use ccr_protocol::{
    ConfigProvider, LlmProvider, TokenizeRequest, TokenizerOptions, TokenizerResult,
    TransformerContext, UnifiedChatRequest,
};
use ccr_tokenizer::TokenizerService;
use ccr_transform::{TransformerPipeline, TransformerService};
use serde_json::Value;

use crate::error::CoreError;
use crate::provider_registry::ProviderRegistry;
use crate::router::decide_model;
use crate::types::{PreparedRequest, RouteDecision, RouterConfig, UsageSnapshot};

/// 默认 tokenizer 名称。
const DEFAULT_TOKENIZER_NAME: &str = "tiktoken:cl100k_base";

/// Core 聚合服务。
pub struct CoreService {
    /// 配置服务。
    config_service: ConfigService,
    /// Provider 注册中心。
    provider_registry: ProviderRegistry,
    /// Tokenizer 服务（有内部缓存，因此需要可变借用）。
    tokenizer_service: TokenizerService,
    /// Transformer 服务。
    transformer_service: TransformerService,
}

/// 响应重写器。
///
/// 说明：
/// - 从 `CoreService` 派生，内部持有 pipeline 与上下文。
/// - 可脱离 `CoreService` 锁独立工作，适合 SSE 流式逐块重写。
#[derive(Clone)]
pub struct ResponseRewriter {
    pipeline: TransformerPipeline,
    context: TransformerContext,
}

impl ResponseRewriter {
    /// 执行响应重写链。
    ///
    /// 顺序：`response_in` -> `response_out`。
    pub async fn rewrite(&self, response: &Value) -> Result<Value, CoreError> {
        let response_in = self
            .pipeline
            .transform_response_in(response, &self.context)
            .await?;
        self.pipeline
            .transform_response_out(&response_in, &self.context)
            .await
            .map_err(CoreError::from)
    }
}

impl CoreService {
    /// 基于配置服务创建 CoreService。
    pub fn new(config_service: ConfigService) -> Self {
        let providers = load_providers_from_config(&config_service);

        Self {
            config_service,
            provider_registry: ProviderRegistry::from_config(&providers),
            tokenizer_service: TokenizerService::with_default_tokenizers(
                TokenizerOptions::default(),
            ),
            transformer_service: TransformerService::new(),
        }
    }

    /// 通过外部组件创建服务（用于测试或定制注入）。
    pub fn with_components(
        config_service: ConfigService,
        provider_registry: ProviderRegistry,
        tokenizer_service: TokenizerService,
        transformer_service: TransformerService,
    ) -> Self {
        Self {
            config_service,
            provider_registry,
            tokenizer_service,
            transformer_service,
        }
    }

    /// 获取 provider 注册中心只读引用。
    pub fn provider_registry(&self) -> &ProviderRegistry {
        &self.provider_registry
    }

    /// 获取配置服务只读引用。
    pub fn config_service(&self) -> &ConfigService {
        &self.config_service
    }

    /// 重新加载 provider 配置。
    pub fn reload_providers(&mut self) {
        let providers = load_providers_from_config(&self.config_service);
        self.provider_registry = ProviderRegistry::from_config(&providers);
    }

    /// 返回当前 provider 列表。
    pub fn providers(&self) -> Vec<LlmProvider> {
        self.provider_registry.providers()
    }

    /// 返回当前完整配置快照。
    pub fn config_snapshot(&self) -> Value {
        self.config_service.get_all()
    }

    /// 对请求做纯 token 统计，不触发路由与转换。
    pub async fn count_tokens(
        &mut self,
        request_body: &Value,
    ) -> Result<TokenizerResult, CoreError> {
        self.count_tokens_for_request(request_body).await
    }

    /// 基于准备结果构建响应重写器。
    pub fn build_response_rewriter(
        &self,
        prepared: &PreparedRequest,
    ) -> Result<ResponseRewriter, CoreError> {
        let pipeline =
            self.build_pipeline_for_provider(&prepared.route_info.provider.transformer)?;
        Ok(ResponseRewriter {
            pipeline,
            context: build_transform_context(&prepared.route_decision, prepared.token_count),
        })
    }

    /// 预处理请求：
    /// 1. token 统计
    /// 2. 路由决策
    /// 3. provider 路由解析
    /// 4. transformer pipeline 执行
    pub async fn prepare_request(
        &mut self,
        request_body: &mut Value,
        last_usage: Option<&UsageSnapshot>,
    ) -> Result<PreparedRequest, CoreError> {
        ensure_object_request(request_body)?;

        // 先统计 token，保持与 TS 路由逻辑一致。
        let token_result = self.count_tokens_for_request(request_body).await?;

        // 做路由决策。
        let router_config = self.load_router_config();
        let route_decision = decide_model(
            request_body,
            token_result.token_count,
            last_usage,
            &router_config,
            &self.provider_registry,
        );

        if route_decision.model.is_empty() {
            return Err(CoreError::MissingModel);
        }

        // 把最终模型写回请求体。
        request_body["model"] = Value::String(route_decision.model.clone());

        // 为统一协议做最小兼容归一化。
        normalize_request_for_protocol(request_body);

        let mut unified_request: UnifiedChatRequest = serde_json::from_value(request_body.clone())
            .map_err(|err| {
                CoreError::RequestInvalid(format!("请求不符合 UnifiedChatRequest: {err}"))
            })?;

        // provider 路由解析。
        let route_info = self.provider_registry.resolve(&route_decision.model)?;

        // 发送给 provider 时 model 只保留目标模型名。
        unified_request.model = route_info.target_model.clone();

        // 根据 provider 配置构建 transformer pipeline。
        let pipeline = self.build_pipeline_for_provider(&route_info.provider.transformer)?;

        // 组织 transformer 上下文，保留决策元信息。
        let transform_context = build_transform_context(&route_decision, token_result.token_count);

        // 执行请求转换 + 鉴权增强。
        let payload = pipeline
            .transform_request_in(&unified_request, &route_info.provider, &transform_context)
            .await?;
        let provider_payload = pipeline
            .apply_auth(&payload, &route_info.provider, &transform_context)
            .await?;

        Ok(PreparedRequest {
            route_decision,
            route_info,
            token_count: token_result.token_count,
            tokenizer_used: token_result.tokenizer_used,
            tokenizer_cached: token_result.cached,
            transformer_chain: pipeline.names(),
            unified_request,
            provider_payload,
        })
    }

    /// 统计请求 token。
    async fn count_tokens_for_request(
        &mut self,
        request_body: &Value,
    ) -> Result<TokenizerResult, CoreError> {
        let tokenize_request = build_tokenize_request(request_body);

        // 尝试匹配 provider/model 的 tokenizer 配置。
        if let Some((provider_name, model_name)) = extract_provider_and_model(request_body) {
            if let Some(tokenizer_config) = self
                .provider_registry
                .get_tokenizer_config_for_model(provider_name, model_name)
            {
                return self
                    .tokenizer_service
                    .count_tokens_with_config(&tokenizer_config, &tokenize_request)
                    .await
                    .map_err(CoreError::from);
            }
        }

        // 回退到默认 tokenizer。
        self.tokenizer_service
            .count_tokens(DEFAULT_TOKENIZER_NAME, &tokenize_request)
            .await
            .map_err(CoreError::from)
    }

    /// 构建 provider 对应 transformer pipeline。
    fn build_pipeline_for_provider(
        &self,
        provider_transformer: &Option<Value>,
    ) -> Result<TransformerPipeline, CoreError> {
        let Some(transformer_value) = provider_transformer.as_ref() else {
            return Ok(TransformerPipeline::default());
        };

        if let Some(use_chain) = extract_transformer_use_chain(transformer_value) {
            return self
                .transformer_service
                .build_pipeline_from_use_chain(use_chain)
                .map_err(CoreError::from);
        }

        Ok(TransformerPipeline::default())
    }

    /// 从配置读取 Router。
    fn load_router_config(&self) -> RouterConfig {
        self.config_service
            .get::<RouterConfig>("Router")
            .or_else(|| self.config_service.get::<RouterConfig>("router"))
            .unwrap_or_default()
    }
}

/// 确保请求体是 JSON 对象。
fn ensure_object_request(request_body: &Value) -> Result<(), CoreError> {
    if request_body.is_object() {
        Ok(())
    } else {
        Err(CoreError::RequestInvalid(
            "请求体必须是 JSON 对象".to_string(),
        ))
    }
}

/// 从配置读取 providers 列表。
fn load_providers_from_config(config_service: &ConfigService) -> Vec<ConfigProvider> {
    config_service
        .get::<Vec<ConfigProvider>>("providers")
        .or_else(|| config_service.get::<Vec<ConfigProvider>>("Providers"))
        .unwrap_or_default()
}

/// 构建 transformer 上下文。
fn build_transform_context(route_decision: &RouteDecision, token_count: u64) -> TransformerContext {
    let mut transform_context = TransformerContext::default();
    transform_context.extra.insert(
        "scenarioType".to_string(),
        Value::String(route_decision.scenario_type.as_str().to_string()),
    );
    transform_context
        .extra
        .insert("tokenCount".to_string(), Value::from(token_count));
    transform_context
}

/// 为统一协议做轻量字段兼容。
///
/// 当前处理：
/// - 若存在 `thinking` 且缺少 `reasoning`，则复制到 `reasoning`。
fn normalize_request_for_protocol(request_body: &mut Value) {
    let Some(map) = request_body.as_object_mut() else {
        return;
    };

    if !map.contains_key("reasoning") {
        if let Some(thinking) = map.get("thinking").cloned() {
            map.insert("reasoning".to_string(), thinking);
        }
    }

    // 兼容 anthropic / custom 工具格式，避免反序列化 UnifiedTool 失败。
    if let Some(tools) = map.get_mut("tools").and_then(Value::as_array_mut) {
        for tool in tools {
            let Some(tool_object) = tool.as_object_mut() else {
                continue;
            };

            // 已是统一格式则无需处理。
            if tool_object.contains_key("function") {
                continue;
            }

            let inferred_name = tool_object
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| tool_object.get("type").and_then(Value::as_str))
                .unwrap_or("tool")
                .to_string();

            let inferred_arguments = tool_object
                .get("input_schema")
                .or_else(|| tool_object.get("inputSchema"))
                .map(|schema| serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string()))
                .unwrap_or_else(|| "{}".to_string());

            tool_object
                .entry("type".to_string())
                .or_insert_with(|| Value::String("function".to_string()));
            tool_object.insert(
                "function".to_string(),
                serde_json::json!({
                    "name": inferred_name,
                    "arguments": inferred_arguments,
                }),
            );
        }
    }
}

/// 从请求里提取 `provider,model`。
fn extract_provider_and_model(request_body: &Value) -> Option<(&str, &str)> {
    let model = request_body.get("model")?.as_str()?;
    let (provider_name, model_name) = model.split_once(',')?;
    Some((provider_name.trim(), model_name.trim()))
}

/// 构建 TokenizeRequest。
fn build_tokenize_request(request_body: &Value) -> TokenizeRequest {
    let messages = request_body
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let system = request_body.get("system").cloned();

    let tools = request_body
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .filter(|list| !list.is_empty());

    TokenizeRequest {
        messages,
        system,
        tools,
    }
}

/// 提取 transformer `use` 链。
///
/// 支持两种写法：
/// - `transformer: ["passthrough", ["maxtoken", {...}]]`
/// - `transformer: { use: [ ... ] }`
fn extract_transformer_use_chain(transformer_value: &Value) -> Option<&Value> {
    if transformer_value.is_array() {
        return Some(transformer_value);
    }

    transformer_value
        .as_object()
        .and_then(|object| object.get("use"))
        .filter(|value| value.is_array())
}

#[cfg(test)]
mod tests {
    use ccr_config::{ConfigOptions, ConfigService};
    use serde_json::json;

    use super::CoreService;
    use crate::types::RouterScenarioType;

    /// 构造仅内存配置服务。
    fn config_service_with(value: serde_json::Value) -> ConfigService {
        ConfigService::new(ConfigOptions {
            use_json_file: false,
            use_env_file: false,
            use_environment_variables: false,
            initial_config: Some(value),
            ..ConfigOptions::default()
        })
        .expect("create config service")
    }

    #[tokio::test]
    async fn test_prepare_request_with_tokenizer_and_transformer_pipeline() {
        let config = json!({
            "providers": [
                {
                    "name": "openai",
                    "api_base_url": "https://api.openai.com/v1/chat/completions",
                    "api_key": "sk-openai",
                    "models": ["gpt-5", "gpt-5-mini"],
                    "tokenizer": {
                        "default": {
                            "tokenizer_type": "tiktoken",
                            "encoding": "cl100k_base"
                        }
                    },
                    "transformer": {
                        "use": [
                            ["maxtoken", {"max_tokens": 2048}],
                            ["sampling", {"temperature": 0.2}]
                        ]
                    }
                }
            ],
            "Router": {
                "default": "openai,gpt-5"
            }
        });

        let mut core_service = CoreService::new(config_service_with(config));

        let mut request_body = json!({
            "model": "OpenAI,GPT-5",
            "messages": [
                {"role": "user", "content": "你好，介绍一下 Rust"}
            ],
            "stream": false
        });

        let prepared = core_service
            .prepare_request(&mut request_body, None)
            .await
            .expect("prepare request");

        assert_eq!(prepared.route_decision.model, "openai,gpt-5");
        assert_eq!(prepared.route_info.target_model, "gpt-5");
        assert_eq!(
            prepared.route_decision.scenario_type,
            RouterScenarioType::Default
        );
        assert!(prepared.token_count > 0);
        assert_eq!(prepared.provider_payload["max_tokens"], json!(2048));
        assert_eq!(prepared.provider_payload["temperature"], json!(0.2));
        assert_eq!(prepared.transformer_chain, vec!["maxtoken", "sampling"]);
    }

    #[tokio::test]
    async fn test_prepare_request_web_search_scenario() {
        let config = json!({
            "providers": [
                {
                    "name": "openai",
                    "api_base_url": "https://api.openai.com/v1/chat/completions",
                    "api_key": "sk-openai",
                    "models": ["gpt-5", "gpt-5-search"]
                }
            ],
            "Router": {
                "default": "openai,gpt-5",
                "webSearch": "openai,gpt-5-search",
                "think": "openai,gpt-5"
            }
        });

        let mut core_service = CoreService::new(config_service_with(config));

        let mut request_body = json!({
            "model": "gpt-5",
            "messages": [{"role": "user", "content": "帮我搜索一下今天的技术新闻"}],
            "tools": [{"type": "web_search_preview"}],
            "thinking": {"type": "enabled"},
            "stream": false
        });

        let prepared = core_service
            .prepare_request(&mut request_body, None)
            .await
            .expect("prepare request");

        assert_eq!(
            prepared.route_decision.scenario_type,
            RouterScenarioType::WebSearch
        );
        assert_eq!(prepared.route_decision.model, "openai,gpt-5-search");
        assert_eq!(prepared.route_info.target_model, "gpt-5-search");
    }
}
