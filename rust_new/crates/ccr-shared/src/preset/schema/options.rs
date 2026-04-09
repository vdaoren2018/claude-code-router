//! 动态选项解析。

use serde_json::json;

use crate::preset::types::{
    DynamicOptions, DynamicOptionsType, FieldOptions, InputOption, PresetConfigSection,
    RequiredInput, UserInputValues,
};

/// 解析动态选项。
pub fn get_dynamic_options(
    dynamic_options: &DynamicOptions,
    preset_config: &PresetConfigSection,
    values: &UserInputValues,
) -> Vec<InputOption> {
    match dynamic_options.source_type {
        DynamicOptionsType::Static => dynamic_options.options.clone().unwrap_or_default(),
        DynamicOptionsType::Providers => {
            let Some(providers) = &preset_config.providers else {
                return vec![];
            };

            providers
                .iter()
                .map(|provider| InputOption {
                    label: provider.name.clone(),
                    value: json!(provider.name),
                    description: Some(provider.api_base_url.clone()),
                    disabled: None,
                    icon: None,
                })
                .collect()
        }
        DynamicOptionsType::Models => {
            let Some(provider_field) = &dynamic_options.provider_field else {
                return vec![];
            };

            let provider_key = provider_field
                .trim()
                .trim_start_matches("#{")
                .trim_end_matches('}')
                .to_string();
            let Some(selected_provider) = values
                .get(&provider_key)
                .and_then(|value| value.as_str())
                .map(|s| s.to_string())
            else {
                return vec![];
            };

            let Some(providers) = &preset_config.providers else {
                return vec![];
            };

            providers
                .iter()
                .find(|provider| provider.name == selected_provider)
                .map(|provider| {
                    provider
                        .models
                        .iter()
                        .map(|model| InputOption {
                            label: model.clone(),
                            value: json!(model),
                            description: None,
                            disabled: None,
                            icon: None,
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        DynamicOptionsType::Custom => {
            // 预留：自定义数据源后续接入。
            vec![]
        }
    }
}

/// 解析字段 options（支持静态与动态两种形态）。
pub fn resolve_options(
    field: &RequiredInput,
    preset_config: &PresetConfigSection,
    values: &UserInputValues,
) -> Vec<InputOption> {
    let Some(options) = &field.options else {
        return vec![];
    };

    match options {
        FieldOptions::Static(list) => list.clone(),
        FieldOptions::Dynamic(dynamic) => get_dynamic_options(dynamic, preset_config, values),
    }
}
