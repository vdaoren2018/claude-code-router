//! Core 路由决策实现。
//!
//! 规则对齐 TS `packages/core/src/utils/router.ts`：
//! 1. 显式 `provider,model` 先做合法性校验并规范化。
//! 2. 长上下文优先于 subagent/background/think。
//! 3. `<CCR-SUBAGENT-MODEL>` 支持内联模型覆盖。
//! 4. Claude Haiku 可路由到 background。
//! 5. web_search 优先级高于 thinking。

use std::sync::LazyLock;

use regex::Regex;
use serde_json::Value;

use crate::provider_registry::ProviderRegistry;
use crate::types::{RouteDecision, RouterConfig, RouterScenarioType, UsageSnapshot};

/// Subagent 模型标签匹配。
static SUBAGENT_MODEL_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)<CCR-SUBAGENT-MODEL>(.*?)</CCR-SUBAGENT-MODEL>")
        .expect("compile subagent model regex")
});

/// 根据请求与上下文做路由决策。
pub fn decide_model(
    request_body: &mut Value,
    token_count: u64,
    last_usage: Option<&UsageSnapshot>,
    router_config: &RouterConfig,
    provider_registry: &ProviderRegistry,
) -> RouteDecision {
    let current_model = request_body
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    // 规则 1：显式 provider,model 优先规范化。
    if current_model.contains(',') {
        if let Some(canonical_model) = provider_registry.canonicalize_explicit_model(&current_model)
        {
            return RouteDecision {
                model: canonical_model,
                scenario_type: RouterScenarioType::Default,
            };
        }

        // 显式模型不合法时保持原值，尽量与 TS 行为对齐。
        return RouteDecision {
            model: current_model,
            scenario_type: RouterScenarioType::Default,
        };
    }

    // 规则 2：长上下文判断。
    let long_context_threshold = router_config.long_context_threshold.unwrap_or(60_000);
    let last_usage_threshold = last_usage
        .map(|usage| usage.input_tokens > long_context_threshold && token_count > 20_000)
        .unwrap_or(false);
    let token_count_threshold = token_count > long_context_threshold;
    if (last_usage_threshold || token_count_threshold)
        && router_config
            .long_context
            .as_ref()
            .is_some_and(|model| !model.is_empty())
    {
        return RouteDecision {
            model: router_config.long_context.clone().unwrap_or_default(),
            scenario_type: RouterScenarioType::LongContext,
        };
    }

    // 规则 3：subagent 模型标签。
    if let Some(model) = extract_subagent_model(request_body) {
        return RouteDecision {
            model,
            scenario_type: RouterScenarioType::Default,
        };
    }

    // 规则 4：Claude Haiku -> background。
    if current_model.contains("claude")
        && current_model.contains("haiku")
        && router_config
            .background
            .as_ref()
            .is_some_and(|model| !model.is_empty())
    {
        return RouteDecision {
            model: router_config.background.clone().unwrap_or_default(),
            scenario_type: RouterScenarioType::Background,
        };
    }

    // 规则 5：web_search 优先于 thinking。
    if has_web_search_tool(request_body)
        && router_config
            .web_search
            .as_ref()
            .is_some_and(|model| !model.is_empty())
    {
        return RouteDecision {
            model: router_config.web_search.clone().unwrap_or_default(),
            scenario_type: RouterScenarioType::WebSearch,
        };
    }

    // 规则 6：thinking -> think。
    if has_thinking_flag(request_body)
        && router_config
            .think
            .as_ref()
            .is_some_and(|model| !model.is_empty())
    {
        return RouteDecision {
            model: router_config.think.clone().unwrap_or_default(),
            scenario_type: RouterScenarioType::Think,
        };
    }

    // 规则 7：默认模型。
    RouteDecision {
        model: router_config
            .default
            .clone()
            .filter(|model| !model.is_empty())
            .unwrap_or(current_model),
        scenario_type: RouterScenarioType::Default,
    }
}

/// 是否使用了 web_search 工具。
fn has_web_search_tool(request_body: &Value) -> bool {
    request_body
        .get("tools")
        .and_then(Value::as_array)
        .is_some_and(|tools| {
            tools.iter().any(|tool| {
                tool.get("type")
                    .and_then(Value::as_str)
                    .is_some_and(|tool_type| tool_type.starts_with("web_search"))
            })
        })
}

/// 是否携带 thinking 标记。
fn has_thinking_flag(request_body: &Value) -> bool {
    request_body
        .get("thinking")
        .is_some_and(|value| !value.is_null())
        || request_body
            .get("reasoning")
            .is_some_and(|value| !value.is_null())
}

/// 提取并清理 `<CCR-SUBAGENT-MODEL>` 标签。
fn extract_subagent_model(request_body: &mut Value) -> Option<String> {
    let system = request_body.get_mut("system")?.as_array_mut()?;
    if system.len() <= 1 {
        return None;
    }

    let text_value = system.get_mut(1)?.get_mut("text")?;
    let text = text_value.as_str()?.to_string();

    if !text.starts_with("<CCR-SUBAGENT-MODEL>") {
        return None;
    }

    let captures = SUBAGENT_MODEL_REGEX.captures(&text)?;
    let model = captures.get(1)?.as_str().trim().to_string();
    if model.is_empty() {
        return None;
    }

    // 清理标签，保留余下 prompt 文本。
    let cleaned_text = SUBAGENT_MODEL_REGEX.replace(&text, "").to_string();
    *text_value = Value::String(cleaned_text);

    Some(model)
}

#[cfg(test)]
mod tests {
    use ccr_protocol::ConfigProvider;
    use serde_json::json;

    use super::decide_model;
    use crate::provider_registry::ProviderRegistry;
    use crate::types::{RouterConfig, RouterScenarioType, UsageSnapshot};

    /// 构建测试 registry。
    fn test_registry() -> ProviderRegistry {
        ProviderRegistry::from_config(&[
            ConfigProvider {
                name: "openai".to_string(),
                api_base_url: "https://api.openai.com/v1/chat/completions".to_string(),
                api_key: "sk-openai".to_string(),
                models: vec!["gpt-5".to_string(), "gpt-5-mini".to_string()],
                transformer: None,
                tokenizer: None,
            },
            ConfigProvider {
                name: "anthropic".to_string(),
                api_base_url: "https://api.anthropic.com/v1/messages".to_string(),
                api_key: "sk-anthropic".to_string(),
                models: vec!["claude-3-5-haiku".to_string()],
                transformer: None,
                tokenizer: None,
            },
        ])
    }

    #[test]
    fn test_explicit_model_canonicalization() {
        let mut body = json!({
            "model": "OpenAI,GPT-5",
            "messages": []
        });

        let decision = decide_model(
            &mut body,
            1,
            None,
            &RouterConfig::default(),
            &test_registry(),
        );

        assert_eq!(decision.model, "openai,gpt-5");
        assert_eq!(decision.scenario_type, RouterScenarioType::Default);
    }

    #[test]
    fn test_long_context_scenario() {
        let mut body = json!({
            "model": "gpt-5",
            "messages": []
        });

        let decision = decide_model(
            &mut body,
            80_000,
            Some(&UsageSnapshot {
                input_tokens: 90_000,
                output_tokens: 0,
                total_tokens: 90_000,
            }),
            &RouterConfig {
                long_context: Some("openai,gpt-5".to_string()),
                long_context_threshold: Some(60_000),
                ..RouterConfig::default()
            },
            &test_registry(),
        );

        assert_eq!(decision.model, "openai,gpt-5");
        assert_eq!(decision.scenario_type, RouterScenarioType::LongContext);
    }

    #[test]
    fn test_web_search_priority_higher_than_think() {
        let mut body = json!({
            "model": "gpt-5",
            "messages": [],
            "tools": [{"type": "web_search_preview"}],
            "thinking": {"type": "enabled"}
        });

        let decision = decide_model(
            &mut body,
            1,
            None,
            &RouterConfig {
                web_search: Some("openai,gpt-5".to_string()),
                think: Some("openai,gpt-5-mini".to_string()),
                ..RouterConfig::default()
            },
            &test_registry(),
        );

        assert_eq!(decision.scenario_type, RouterScenarioType::WebSearch);
        assert_eq!(decision.model, "openai,gpt-5");
    }

    #[test]
    fn test_extract_subagent_model_and_clean_text() {
        let mut body = json!({
            "model": "gpt-5",
            "messages": [],
            "system": [
                {"type": "text", "text": "ignored"},
                {"type": "text", "text": "<CCR-SUBAGENT-MODEL>openai,gpt-5-mini</CCR-SUBAGENT-MODEL>你是子代理"}
            ]
        });

        let decision = decide_model(
            &mut body,
            10,
            None,
            &RouterConfig::default(),
            &test_registry(),
        );

        assert_eq!(decision.model, "openai,gpt-5-mini");
        assert_eq!(decision.scenario_type, RouterScenarioType::Default);
        assert_eq!(body["system"][1]["text"], json!("你是子代理"));
    }
}
