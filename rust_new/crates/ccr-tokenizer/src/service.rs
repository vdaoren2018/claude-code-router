//! Tokenizer 服务实现。

use std::collections::HashMap;

use ccr_protocol::{
    ProviderTokenizerConfig, TokenizeRequest, Tokenizer, TokenizerConfig, TokenizerOptions,
    TokenizerResult, TokenizerType,
};

use crate::error::TokenizerError;
use crate::tokenizers::{ApiTokenizer, HuggingFaceTokenizer, SimpleTokenizer};

/// Tokenizer 服务。
///
/// 核心职责：
/// 1. 按名称管理 tokenizer 实例。
/// 2. 支持 fallback 执行链。
/// 3. 支持基于配置动态创建 tokenizer。
/// 4. 支持简单缓存，避免重复统计。
pub struct TokenizerService {
    tokenizers: HashMap<String, Box<dyn Tokenizer>>,
    fallback_name: Option<String>,
    cache_enabled: bool,
    cache: HashMap<String, u64>,
}

impl Default for TokenizerService {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl TokenizerService {
    /// 创建服务。
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            tokenizers: HashMap::new(),
            fallback_name: None,
            cache_enabled: options.cache_enabled.unwrap_or(false),
            cache: HashMap::new(),
        }
    }

    /// 创建并注册默认 tokenizer。
    pub fn with_default_tokenizers(options: TokenizerOptions) -> Self {
        let mut service = Self::new(options);

        // 默认 tiktoken 占位实现。
        service.register(
            "tiktoken:cl100k_base",
            Box::new(SimpleTokenizer::new("cl100k_base")),
        );

        // 默认 fallback。
        service.set_fallback("tiktoken:cl100k_base");
        service
    }

    /// 注册 tokenizer。
    pub fn register(&mut self, name: impl Into<String>, tokenizer: Box<dyn Tokenizer>) {
        self.tokenizers.insert(name.into(), tokenizer);
    }

    /// 是否存在 tokenizer。
    pub fn has(&self, name: &str) -> bool {
        self.tokenizers.contains_key(name)
    }

    /// 设置 fallback。
    pub fn set_fallback(&mut self, name: impl Into<String>) {
        self.fallback_name = Some(name.into());
    }

    /// 清空缓存。
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// 通过名称统计 token。
    pub async fn count_tokens(
        &mut self,
        name: &str,
        request: &TokenizeRequest,
    ) -> Result<TokenizerResult, TokenizerError> {
        let cache_key = self.build_cache_key(name, request)?;
        if self.cache_enabled {
            if let Some(count) = self.cache.get(&cache_key) {
                return Ok(TokenizerResult {
                    token_count: *count,
                    tokenizer_used: self.resolve_candidate_name(name)?,
                    cached: true,
                });
            }
        }

        let candidates = self.resolve_candidates(name)?;
        let mut last_error: Option<TokenizerError> = None;

        for candidate in candidates {
            match self.count_tokens_with_one(&candidate, request).await {
                Ok(count) => {
                    if self.cache_enabled {
                        self.cache.insert(cache_key.clone(), count);
                    }
                    return Ok(TokenizerResult {
                        token_count: count,
                        tokenizer_used: candidate,
                        cached: false,
                    });
                }
                Err(err) => {
                    last_error = Some(err);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| TokenizerError::NotFound(name.to_string())))
    }

    /// 按配置统计 token。
    ///
    /// 说明：
    /// - 若配置对应 tokenizer 不存在，会自动创建并注册。
    /// - 创建后走统一 `count_tokens` 流程，因此同样支持缓存与 fallback。
    pub async fn count_tokens_with_config(
        &mut self,
        config: &TokenizerConfig,
        request: &TokenizeRequest,
    ) -> Result<TokenizerResult, TokenizerError> {
        let name = self.ensure_tokenizer_by_config(config)?;
        self.count_tokens(&name, request).await
    }

    /// 从 provider tokenizer 配置中选择最终 tokenizer。
    pub fn get_tokenizer_config_for_model(
        provider_config: &ProviderTokenizerConfig,
        model_name: &str,
    ) -> Option<TokenizerConfig> {
        if let Some(models) = &provider_config.models {
            if let Some(config) = models.get(model_name) {
                return Some(config.clone());
            }
        }

        provider_config.default.clone()
    }

    /// 根据配置保证 tokenizer 已创建，返回注册名。
    fn ensure_tokenizer_by_config(
        &mut self,
        config: &TokenizerConfig,
    ) -> Result<String, TokenizerError> {
        let key = config_to_key(config);

        if !self.tokenizers.contains_key(&key) {
            let tokenizer = create_tokenizer_from_config(config)?;
            self.register(key.clone(), tokenizer);
        }

        Ok(key)
    }

    /// 构造缓存 key。
    fn build_cache_key(
        &self,
        name: &str,
        request: &TokenizeRequest,
    ) -> Result<String, TokenizerError> {
        let serialized = serde_json::to_string(request)?;
        Ok(format!("{name}:{serialized}"))
    }

    /// 解析当前最终候选名称。
    fn resolve_candidate_name(&self, name: &str) -> Result<String, TokenizerError> {
        if self.tokenizers.contains_key(name) {
            return Ok(name.to_string());
        }

        self.fallback_name
            .clone()
            .ok_or_else(|| TokenizerError::NotFound(name.to_string()))
    }

    /// 生成候选执行列表。
    fn resolve_candidates(&self, name: &str) -> Result<Vec<String>, TokenizerError> {
        let mut candidates = Vec::new();

        if self.tokenizers.contains_key(name) {
            candidates.push(name.to_string());
        }

        if let Some(fallback) = &self.fallback_name {
            if !candidates.contains(fallback) {
                candidates.push(fallback.clone());
            }
        }

        if candidates.is_empty() {
            return Err(TokenizerError::NotFound(name.to_string()));
        }

        Ok(candidates)
    }

    /// 用单个 tokenizer 执行统计。
    async fn count_tokens_with_one(
        &mut self,
        name: &str,
        request: &TokenizeRequest,
    ) -> Result<u64, TokenizerError> {
        let tokenizer = self
            .tokenizers
            .get_mut(name)
            .ok_or_else(|| TokenizerError::NotFound(name.to_string()))?;

        if !tokenizer.is_initialized() {
            tokenizer
                .initialize()
                .await
                .map_err(|message| TokenizerError::InitFailed {
                    name: name.to_string(),
                    message,
                })?;
        }

        tokenizer
            .count_tokens(request)
            .await
            .map_err(|message| TokenizerError::Failed {
                name: name.to_string(),
                message,
            })
    }
}

/// 把配置转成可复用 key。
fn config_to_key(config: &TokenizerConfig) -> String {
    match config.tokenizer_type {
        TokenizerType::Tiktoken => {
            format!(
                "tiktoken:{}",
                config
                    .encoding
                    .clone()
                    .unwrap_or_else(|| "cl100k_base".to_string())
            )
        }
        TokenizerType::Huggingface => {
            format!(
                "hf:{}",
                config
                    .model
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string())
            )
        }
        TokenizerType::Api => {
            format!(
                "api:{}",
                config.url.clone().unwrap_or_else(|| "unknown".to_string())
            )
        }
    }
}

/// 根据配置创建 tokenizer。
fn create_tokenizer_from_config(
    config: &TokenizerConfig,
) -> Result<Box<dyn Tokenizer>, TokenizerError> {
    let tokenizer: Box<dyn Tokenizer> = match config.tokenizer_type {
        TokenizerType::Tiktoken => Box::new(SimpleTokenizer::new(
            config
                .encoding
                .clone()
                .unwrap_or_else(|| "cl100k_base".to_string()),
        )),
        TokenizerType::Huggingface => {
            Box::new(HuggingFaceTokenizer::new(config.model.clone().ok_or_else(
                || TokenizerError::InvalidConfig("huggingface 缺少 model".to_string()),
            )?))
        }
        TokenizerType::Api => Box::new(ApiTokenizer::new(
            config.url.clone().ok_or_else(|| {
                TokenizerError::InvalidConfig("api tokenizer 缺少 url".to_string())
            })?,
            config.api_key.clone(),
            config.request_format.clone(),
            config.response_field.clone(),
            config.headers.clone(),
        )),
    };

    Ok(tokenizer)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use ccr_protocol::{
        TokenizeRequest, Tokenizer, TokenizerConfig, TokenizerOptions, TokenizerType,
    };
    use serde_json::json;

    use super::TokenizerService;

    /// 测试用 tokenizer。
    struct MockTokenizer {
        name: String,
        fail: bool,
        initialized: bool,
        calls: Arc<AtomicUsize>,
    }

    impl MockTokenizer {
        fn new(name: impl Into<String>, fail: bool, calls: Arc<AtomicUsize>) -> Self {
            Self {
                name: name.into(),
                fail,
                initialized: false,
                calls,
            }
        }
    }

    #[async_trait]
    impl Tokenizer for MockTokenizer {
        fn tokenizer_type(&self) -> &str {
            "mock"
        }

        fn name(&self) -> &str {
            &self.name
        }

        async fn initialize(&mut self) -> Result<(), String> {
            self.initialized = true;
            Ok(())
        }

        async fn count_tokens(&self, _request: &TokenizeRequest) -> Result<u64, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                Err("mock failed".to_string())
            } else {
                Ok(42)
            }
        }

        fn is_initialized(&self) -> bool {
            self.initialized
        }

        fn dispose(&mut self) {
            self.initialized = false;
        }
    }

    fn sample_request() -> TokenizeRequest {
        TokenizeRequest {
            messages: vec![json!({"role":"user","content":"hello world"})],
            system: Some(json!("system prompt")),
            tools: None,
        }
    }

    #[tokio::test]
    async fn test_count_with_fallback() {
        let mut service = TokenizerService::new(TokenizerOptions::default());

        let primary_calls = Arc::new(AtomicUsize::new(0));
        let fallback_calls = Arc::new(AtomicUsize::new(0));

        service.register(
            "primary",
            Box::new(MockTokenizer::new("primary", true, primary_calls.clone())),
        );
        service.register(
            "fallback",
            Box::new(MockTokenizer::new(
                "fallback",
                false,
                fallback_calls.clone(),
            )),
        );
        service.set_fallback("fallback");

        let result = service
            .count_tokens("primary", &sample_request())
            .await
            .expect("count with fallback");

        assert_eq!(result.token_count, 42);
        assert_eq!(result.tokenizer_used, "fallback");
        assert_eq!(primary_calls.load(Ordering::SeqCst), 1);
        assert_eq!(fallback_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_cache_enabled() {
        let mut service = TokenizerService::new(TokenizerOptions {
            cache_enabled: Some(true),
            cache_size: None,
            timeout: None,
        });

        let calls = Arc::new(AtomicUsize::new(0));
        service.register(
            "cache-test",
            Box::new(MockTokenizer::new("cache-test", false, calls.clone())),
        );

        let first = service
            .count_tokens("cache-test", &sample_request())
            .await
            .expect("first count");
        let second = service
            .count_tokens("cache-test", &sample_request())
            .await
            .expect("second count");

        assert!(!first.cached);
        assert!(second.cached);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_count_with_tiktoken_config() {
        let mut service = TokenizerService::with_default_tokenizers(TokenizerOptions::default());

        let config = TokenizerConfig {
            tokenizer_type: TokenizerType::Tiktoken,
            encoding: Some("cl100k_base".to_string()),
            model: None,
            url: None,
            api_key: None,
            request_format: None,
            response_field: None,
            headers: None,
            fallback: None,
        };

        let result = service
            .count_tokens_with_config(&config, &sample_request())
            .await
            .expect("count with config");

        assert!(result.token_count > 0);
        assert!(!result.tokenizer_used.is_empty());
    }
}
