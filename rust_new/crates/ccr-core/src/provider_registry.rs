//! Provider 注册中心。
//!
//! 关键职责：
//! 1. 基于配置初始化 Provider 与模型路由表。
//! 2. 提供大小写不敏感的模型解析。
//! 3. 暴露 provider tokenizer 配置读取能力。

use std::collections::HashMap;

use ccr_protocol::{
    ConfigProvider, LlmProvider, ModelRoute, ProviderTokenizerConfig, RequestRouteInfo,
    TokenizerConfig,
};
use ccr_tokenizer::TokenizerService;

use crate::error::CoreError;

/// Provider 存储单元。
#[derive(Debug, Clone)]
struct ProviderEntry {
    /// Provider 协议对象。
    provider: LlmProvider,
    /// Tokenizer 配置（可选）。
    tokenizer: Option<ProviderTokenizerConfig>,
}

/// Provider 注册中心。
#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    /// Provider 主表（key 为规范化名称，即配置中的原始大小写名）。
    providers: HashMap<String, ProviderEntry>,
    /// Provider 名称大小写别名表（key 为 lower，value 为规范名）。
    provider_name_alias: HashMap<String, String>,
    /// 模型路由表（key 为 lower(model) 或 lower(provider,model)）。
    routes: HashMap<String, ModelRoute>,
}

impl ProviderRegistry {
    /// 从配置列表创建注册中心。
    pub fn from_config(providers: &[ConfigProvider]) -> Self {
        let mut registry = Self::default();
        for provider in providers {
            registry.register_from_config(provider);
        }
        registry
    }

    /// 注册一个配置 provider（含 tokenizer 信息）。
    pub fn register_from_config(&mut self, provider: &ConfigProvider) {
        let llm_provider = LlmProvider {
            name: provider.name.clone(),
            base_url: provider.api_base_url.clone(),
            api_key: provider.api_key.clone(),
            models: provider.models.clone(),
            transformer: provider.transformer.clone(),
        };

        let tokenizer = provider.tokenizer.as_ref().and_then(|value| {
            serde_json::from_value::<ProviderTokenizerConfig>(value.clone()).ok()
        });

        self.register_internal(llm_provider, tokenizer);
    }

    /// 注册 provider（兼容旧骨架接口，不含 tokenizer 信息）。
    pub fn register(&mut self, provider: LlmProvider) {
        self.register_internal(provider, None);
    }

    /// 返回当前所有 provider。
    pub fn providers(&self) -> Vec<LlmProvider> {
        self.providers
            .values()
            .map(|entry| entry.provider.clone())
            .collect()
    }

    /// 通过名称获取 provider（大小写不敏感）。
    pub fn get_provider(&self, provider_name: &str) -> Option<&LlmProvider> {
        let canonical_name = self
            .provider_name_alias
            .get(&provider_name.to_ascii_lowercase())?;
        self.providers
            .get(canonical_name)
            .map(|entry| &entry.provider)
    }

    /// 对显式模型 `provider,model` 做规范化。
    ///
    /// 返回值示例：`OpenAI,GPT-5` -> `openai,gpt-5`（以配置中的大小写为准）。
    pub fn canonicalize_explicit_model(&self, full_model: &str) -> Option<String> {
        let (provider_name, model_name) = split_provider_model(full_model)?;

        let canonical_provider_name = self
            .provider_name_alias
            .get(&provider_name.to_ascii_lowercase())?
            .clone();
        let provider_entry = self.providers.get(&canonical_provider_name)?;

        let canonical_model = provider_entry
            .provider
            .models
            .iter()
            .find(|model| model.eq_ignore_ascii_case(model_name))?
            .clone();

        Some(format!(
            "{},{}",
            provider_entry.provider.name, canonical_model
        ))
    }

    /// 路由解析。
    pub fn resolve(&self, model_name: &str) -> Result<RequestRouteInfo, CoreError> {
        let route = self.resolve_route(model_name)?;
        let provider_entry = self
            .providers
            .get(&route.provider)
            .ok_or_else(|| CoreError::ProviderNotFound(route.provider.clone()))?;

        Ok(RequestRouteInfo {
            provider: provider_entry.provider.clone(),
            original_model: model_name.to_string(),
            target_model: route.model.clone(),
        })
    }

    /// 获取某 provider+model 对应 tokenizer 配置。
    pub fn get_tokenizer_config_for_model(
        &self,
        provider_name: &str,
        model_name: &str,
    ) -> Option<TokenizerConfig> {
        let provider_entry = self
            .provider_name_alias
            .get(&provider_name.to_ascii_lowercase())
            .and_then(|canonical_name| self.providers.get(canonical_name))?;

        let provider_tokenizer = provider_entry.tokenizer.as_ref()?;
        TokenizerService::get_tokenizer_config_for_model(provider_tokenizer, model_name)
    }

    /// 注册内部实现。
    fn register_internal(
        &mut self,
        provider: LlmProvider,
        tokenizer: Option<ProviderTokenizerConfig>,
    ) {
        let provider_name = provider.name.clone();

        // 先建立 provider 名称别名，支持后续大小写无关解析。
        self.provider_name_alias
            .insert(provider_name.to_ascii_lowercase(), provider_name.clone());

        // 建立模型路由。
        for model in &provider.models {
            let route = ModelRoute {
                provider: provider_name.clone(),
                model: model.clone(),
                full_model: format!("{},{}", provider_name, model),
            };

            // 1) 显式 full model 路由。
            self.routes
                .insert(route.full_model.to_ascii_lowercase(), route.clone());

            // 2) 裸模型路由：保持“先注册优先”。
            self.routes
                .entry(model.to_ascii_lowercase())
                .or_insert(route);
        }

        self.providers.insert(
            provider_name,
            ProviderEntry {
                provider,
                tokenizer,
            },
        );
    }

    /// 路由表解析（内部）。
    fn resolve_route(&self, model_name: &str) -> Result<ModelRoute, CoreError> {
        // 显式 provider,model 优先走规范化路径。
        if model_name.contains(',') {
            if let Some(canonical_full_model) = self.canonicalize_explicit_model(model_name) {
                if let Some(route) = self.routes.get(&canonical_full_model.to_ascii_lowercase()) {
                    return Ok(route.clone());
                }
            }
        }

        // 常规路径：按 lower key 查找。
        self.routes
            .get(&model_name.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| CoreError::RouteNotFound(model_name.to_string()))
    }
}

/// 切分 `provider,model`。
fn split_provider_model(full_model: &str) -> Option<(&str, &str)> {
    let (provider_name, model_name) = full_model.split_once(',')?;
    let provider_name = provider_name.trim();
    let model_name = model_name.trim();

    if provider_name.is_empty() || model_name.is_empty() {
        return None;
    }

    Some((provider_name, model_name))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ccr_protocol::{ConfigProvider, TokenizerType};
    use serde_json::json;

    use super::ProviderRegistry;

    /// 构造测试 provider 配置。
    fn sample_provider_config() -> ConfigProvider {
        ConfigProvider {
            name: "openai".to_string(),
            api_base_url: "https://api.openai.com/v1/chat/completions".to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["gpt-5".to_string(), "gpt-5-mini".to_string()],
            transformer: None,
            tokenizer: Some(json!({
                "default": {
                    "tokenizer_type": "tiktoken",
                    "encoding": "cl100k_base"
                },
                "models": {
                    "gpt-5-mini": {
                        "tokenizer_type": "huggingface",
                        "model": "Qwen/Qwen3-4B"
                    }
                }
            })),
        }
    }

    #[test]
    fn test_case_insensitive_full_model_resolve() {
        let registry = ProviderRegistry::from_config(&[sample_provider_config()]);

        let route = registry
            .resolve("OpenAI,GPT-5")
            .expect("resolve mixed-case full model");

        assert_eq!(route.provider.name, "openai");
        assert_eq!(route.target_model, "gpt-5");
    }

    #[test]
    fn test_bare_model_route_resolve() {
        let registry = ProviderRegistry::from_config(&[sample_provider_config()]);

        let route = registry.resolve("GPT-5").expect("resolve bare model");
        assert_eq!(route.provider.name, "openai");
        assert_eq!(route.target_model, "gpt-5");
    }

    #[test]
    fn test_get_tokenizer_config_for_model() {
        let registry = ProviderRegistry::from_config(&[sample_provider_config()]);

        let default_config = registry
            .get_tokenizer_config_for_model("openai", "gpt-5")
            .expect("default tokenizer config");
        assert_eq!(default_config.tokenizer_type, TokenizerType::Tiktoken);

        let model_specific_config = registry
            .get_tokenizer_config_for_model("openai", "gpt-5-mini")
            .expect("model tokenizer config");
        assert_eq!(
            model_specific_config.tokenizer_type,
            TokenizerType::Huggingface
        );

        // 验证 headers 这种 map 字段在本模块不会被破坏（回归断言）。
        let map = BTreeMap::from([("x-test".to_string(), "1".to_string())]);
        assert_eq!(map.get("x-test"), Some(&"1".to_string()));
    }
}
