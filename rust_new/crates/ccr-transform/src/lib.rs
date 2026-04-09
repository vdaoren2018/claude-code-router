//! Transformer 注册与执行服务。
//!
//! 该模块在 Phase 2 中从“骨架”升级为“可执行版本”，
//! 主要提供三类能力：
//! 1. Transformer 工厂注册。
//! 2. use 链解析与 pipeline 构建。
//! 3. 请求/响应管线执行。

use std::collections::HashMap;
use std::sync::Arc;

use ccr_protocol::{LlmProvider, Transformer, TransformerContext, UnifiedChatRequest};
use serde_json::Value;
use thiserror::Error;

/// Transformer 工厂签名。
///
/// 说明：
/// - 输入为可选 options（兼容 TS 的 `[name, options]` 形式）。
/// - 输出为可执行的 Transformer 实例。
pub type TransformerFactory =
    Arc<dyn Fn(Option<Value>) -> Result<Arc<dyn Transformer>, TransformError> + Send + Sync>;

/// Transformer pipeline 中的单个声明。
#[derive(Debug, Clone, PartialEq)]
pub enum TransformerSpec {
    /// 仅声明名称：`"openrouter"`
    Name(String),
    /// 名称 + options：`["maxtoken", {"max_tokens": 8192}]`
    WithOptions { name: String, options: Value },
}

impl TransformerSpec {
    /// 获取 transformer 名称。
    fn name(&self) -> &str {
        match self {
            Self::Name(name) => name,
            Self::WithOptions { name, .. } => name,
        }
    }

    /// 获取 options。
    fn options(&self) -> Option<Value> {
        match self {
            Self::Name(_) => None,
            Self::WithOptions { options, .. } => Some(options.clone()),
        }
    }
}

/// 执行阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformStage {
    RequestIn,
    RequestOut,
    ResponseIn,
    ResponseOut,
    Auth,
}

/// 转换器层错误。
#[derive(Debug, Error)]
pub enum TransformError {
    #[error("Transformer 不存在: {0}")]
    TransformerNotFound(String),

    #[error("use 链格式非法: {0}")]
    InvalidUseChain(String),

    #[error("Transformer 工厂初始化失败: {name} -> {message}")]
    FactoryFailed { name: String, message: String },

    #[error("序列化失败: {0}")]
    Serialize(#[from] serde_json::Error),

    #[error("执行失败: {name} @ {stage:?} -> {message}")]
    Execute {
        name: String,
        stage: TransformStage,
        message: String,
    },
}

/// 运行时 pipeline。
#[derive(Default, Clone)]
pub struct TransformerPipeline {
    chain: Vec<Arc<dyn Transformer>>,
}

impl TransformerPipeline {
    /// 创建 pipeline。
    pub fn new(chain: Vec<Arc<dyn Transformer>>) -> Self {
        Self { chain }
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// 返回 pipeline 中的 transformer 名称列表。
    pub fn names(&self) -> Vec<String> {
        self.chain
            .iter()
            .map(|transformer| transformer.name().to_string())
            .collect()
    }

    /// 执行请求入站链。
    ///
    /// 关键点：
    /// - 每个 transformer 都拿到“统一请求”。
    /// - 每步产生 provider payload。
    /// - 尝试通过 `transform_request_out` 还原回统一请求，供下一步继续处理。
    pub async fn transform_request_in(
        &self,
        request: &UnifiedChatRequest,
        provider: &LlmProvider,
        context: &TransformerContext,
    ) -> Result<Value, TransformError> {
        if self.chain.is_empty() {
            return serde_json::to_value(request).map_err(TransformError::from);
        }

        let mut current_request = request.clone();
        let mut payload = serde_json::to_value(request)?;

        for transformer in &self.chain {
            payload = transformer
                .transform_request_in(&current_request, provider, context)
                .await
                .map_err(|message| TransformError::Execute {
                    name: transformer.name().to_string(),
                    stage: TransformStage::RequestIn,
                    message,
                })?;

            // 尝试把中间 payload 还原成统一请求，
            // 如果还原失败则保留上一步统一请求继续传递。
            if let Ok(next_request) = transformer.transform_request_out(&payload, context).await {
                current_request = next_request;
            }
        }

        Ok(payload)
    }

    /// 执行响应入站链（顺序）。
    pub async fn transform_response_in(
        &self,
        response: &Value,
        context: &TransformerContext,
    ) -> Result<Value, TransformError> {
        let mut current = response.clone();

        for transformer in &self.chain {
            current = transformer
                .transform_response_in(&current, context)
                .await
                .map_err(|message| TransformError::Execute {
                    name: transformer.name().to_string(),
                    stage: TransformStage::ResponseIn,
                    message,
                })?;
        }

        Ok(current)
    }

    /// 执行响应出站链（逆序）。
    pub async fn transform_response_out(
        &self,
        response: &Value,
        context: &TransformerContext,
    ) -> Result<Value, TransformError> {
        let mut current = response.clone();

        for transformer in self.chain.iter().rev() {
            current = transformer
                .transform_response_out(&current, context)
                .await
                .map_err(|message| TransformError::Execute {
                    name: transformer.name().to_string(),
                    stage: TransformStage::ResponseOut,
                    message,
                })?;
        }

        Ok(current)
    }

    /// 执行鉴权增强链。
    pub async fn apply_auth(
        &self,
        payload: &Value,
        provider: &LlmProvider,
        context: &TransformerContext,
    ) -> Result<Value, TransformError> {
        let mut current = payload.clone();

        for transformer in &self.chain {
            current = transformer
                .auth(&current, provider, context)
                .await
                .map_err(|message| TransformError::Execute {
                    name: transformer.name().to_string(),
                    stage: TransformStage::Auth,
                    message,
                })?;
        }

        Ok(current)
    }
}

/// Transformer 服务。
pub struct TransformerService {
    registry: HashMap<String, TransformerFactory>,
}

impl Default for TransformerService {
    fn default() -> Self {
        let mut service = Self {
            registry: HashMap::new(),
        };
        service.register_builtin_transformers();
        service
    }
}

impl TransformerService {
    /// 创建服务。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册工厂。
    pub fn register_factory(&mut self, name: impl Into<String>, factory: TransformerFactory) {
        self.registry.insert(name.into(), factory);
    }

    /// 注册固定实例。
    ///
    /// 该模式适合无需 options 的 transformer。
    pub fn register_instance(
        &mut self,
        name: impl Into<String>,
        transformer: Arc<dyn Transformer>,
    ) {
        let name_string = name.into();
        let shared = transformer.clone();
        self.register_factory(name_string, Arc::new(move |_options| Ok(shared.clone())));
    }

    /// 是否已注册。
    pub fn has(&self, name: &str) -> bool {
        self.registry.contains_key(name)
    }

    /// 获取已注册名称。
    pub fn names(&self) -> Vec<String> {
        self.registry.keys().cloned().collect()
    }

    /// 从 spec 构建 pipeline。
    pub fn build_pipeline(
        &self,
        specs: &[TransformerSpec],
    ) -> Result<TransformerPipeline, TransformError> {
        let mut chain = Vec::with_capacity(specs.len());

        for spec in specs {
            let name = spec.name().to_string();
            let factory = self
                .registry
                .get(&name)
                .ok_or_else(|| TransformError::TransformerNotFound(name.clone()))?;

            let transformer =
                factory(spec.options()).map_err(|err| TransformError::FactoryFailed {
                    name,
                    message: err.to_string(),
                })?;
            chain.push(transformer);
        }

        Ok(TransformerPipeline::new(chain))
    }

    /// 从 `use` 配置值直接构建 pipeline。
    pub fn build_pipeline_from_use_chain(
        &self,
        use_chain_value: &Value,
    ) -> Result<TransformerPipeline, TransformError> {
        let specs = Self::parse_use_chain(use_chain_value)?;
        self.build_pipeline(&specs)
    }

    /// 解析 `use` 链。
    ///
    /// 支持：
    /// - `"name"`
    /// - `["name", {...}]`
    pub fn parse_use_chain(
        use_chain_value: &Value,
    ) -> Result<Vec<TransformerSpec>, TransformError> {
        let list = use_chain_value
            .as_array()
            .ok_or_else(|| TransformError::InvalidUseChain("use 字段必须是数组".to_string()))?;

        let mut specs = Vec::with_capacity(list.len());

        for item in list {
            if let Some(name) = item.as_str() {
                specs.push(TransformerSpec::Name(name.to_string()));
                continue;
            }

            if let Some(pair) = item.as_array() {
                if pair.len() == 2 {
                    if let Some(name) = pair[0].as_str() {
                        specs.push(TransformerSpec::WithOptions {
                            name: name.to_string(),
                            options: pair[1].clone(),
                        });
                        continue;
                    }
                }
            }

            return Err(TransformError::InvalidUseChain(format!(
                "非法 use 项: {item}"
            )));
        }

        Ok(specs)
    }

    /// 注册默认内置 transformer。
    fn register_builtin_transformers(&mut self) {
        self.register_instance("passthrough", Arc::new(PassthroughTransformer));

        self.register_factory(
            "maxtoken",
            Arc::new(|options| {
                let max_tokens = options
                    .as_ref()
                    .and_then(|value| value.get("max_tokens"))
                    .and_then(Value::as_u64)
                    .unwrap_or(8192);
                Ok(Arc::new(MaxTokenTransformer { max_tokens }) as Arc<dyn Transformer>)
            }),
        );

        self.register_factory(
            "sampling",
            Arc::new(|options| {
                let temperature = options
                    .as_ref()
                    .and_then(|value| value.get("temperature"))
                    .and_then(Value::as_f64)
                    .unwrap_or(1.0);
                Ok(Arc::new(SamplingTransformer { temperature }) as Arc<dyn Transformer>)
            }),
        );
    }
}

/// 透传 transformer。
struct PassthroughTransformer;

impl Transformer for PassthroughTransformer {
    fn name(&self) -> &str {
        "passthrough"
    }
}

/// MaxToken transformer。
struct MaxTokenTransformer {
    max_tokens: u64,
}

#[async_trait::async_trait]
impl Transformer for MaxTokenTransformer {
    fn name(&self) -> &str {
        "maxtoken"
    }

    async fn transform_request_in(
        &self,
        request: &UnifiedChatRequest,
        _provider: &LlmProvider,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        let mut value = serde_json::to_value(request).map_err(|e| e.to_string())?;
        let Some(map) = value.as_object_mut() else {
            return Ok(value);
        };
        map.insert("max_tokens".to_string(), Value::from(self.max_tokens));
        Ok(value)
    }
}

/// Sampling transformer。
struct SamplingTransformer {
    temperature: f64,
}

#[async_trait::async_trait]
impl Transformer for SamplingTransformer {
    fn name(&self) -> &str {
        "sampling"
    }

    async fn transform_request_in(
        &self,
        request: &UnifiedChatRequest,
        _provider: &LlmProvider,
        _context: &TransformerContext,
    ) -> Result<Value, String> {
        let mut value = serde_json::to_value(request).map_err(|e| e.to_string())?;
        let Some(map) = value.as_object_mut() else {
            return Ok(value);
        };
        map.insert("temperature".to_string(), Value::from(self.temperature));
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::TransformerService;
    use ccr_protocol::{LlmProvider, TransformerContext, UnifiedChatRequest, UnifiedMessage};

    /// 构造测试请求。
    fn test_request() -> UnifiedChatRequest {
        UnifiedChatRequest {
            messages: vec![UnifiedMessage {
                role: "user".to_string(),
                content: json!("hello"),
                tool_calls: None,
                tool_call_id: None,
                cache_control: None,
                thinking: None,
            }],
            model: "openai,gpt-5".to_string(),
            max_tokens: None,
            temperature: None,
            stream: Some(false),
            tools: None,
            tool_choice: None,
            reasoning: None,
        }
    }

    /// 构造测试 provider。
    fn test_provider() -> LlmProvider {
        LlmProvider {
            name: "openai".to_string(),
            base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-5".to_string()],
            transformer: None,
        }
    }

    #[test]
    fn test_parse_use_chain() {
        let chain = json!(["passthrough", ["maxtoken", {"max_tokens": 4096}]]);
        let specs = TransformerService::parse_use_chain(&chain).expect("parse use chain");
        assert_eq!(specs.len(), 2);
    }

    #[tokio::test]
    async fn test_pipeline_with_maxtoken() {
        let service = TransformerService::new();
        let chain = json!(["passthrough", ["maxtoken", {"max_tokens": 4096}]]);
        let pipeline = service
            .build_pipeline_from_use_chain(&chain)
            .expect("build pipeline");

        let payload = pipeline
            .transform_request_in(
                &test_request(),
                &test_provider(),
                &TransformerContext::default(),
            )
            .await
            .expect("transform request");

        assert_eq!(payload["max_tokens"], json!(4096));
    }

    #[test]
    fn test_unknown_transformer() {
        let service = TransformerService::new();
        let chain = json!(["not-exists"]);
        let result = service.build_pipeline_from_use_chain(&chain);
        assert!(result.is_err());
    }
}
