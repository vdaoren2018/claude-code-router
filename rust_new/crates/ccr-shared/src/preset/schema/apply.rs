//! 模板替换、映射应用与 Manifest 装载。

use std::path::Path;

use serde_json::{Map, Value};

use super::{evaluate_conditions, set_value_by_path};
use crate::preset::types::{
    ConfigMapping, ManifestFile, PresetConfigSection, PresetFile, UserInputValues,
};

/// 模板变量替换。
///
/// 语法：`#{variable}`。
pub fn replace_template_variables(template: &Value, values: &UserInputValues) -> Value {
    match template {
        Value::Null | Value::Bool(_) | Value::Number(_) => template.clone(),
        Value::String(raw) => {
            let mut replaced = raw.clone();
            for (key, value) in values {
                let placeholder = format!("#{{{key}}}");
                let as_string = value
                    .as_str()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| value.to_string());
                replaced = replaced.replace(&placeholder, &as_string);
            }
            Value::String(replaced)
        }
        Value::Array(list) => Value::Array(
            list.iter()
                .map(|item| replace_template_variables(item, values))
                .collect(),
        ),
        Value::Object(map) => {
            let mut result = Map::new();
            for (key, value) in map {
                result.insert(key.clone(), replace_template_variables(value, values));
            }
            Value::Object(result)
        }
    }
}

/// 应用映射规则到配置。
pub fn apply_config_mappings(
    mappings: &[ConfigMapping],
    values: &UserInputValues,
    config: &PresetConfigSection,
) -> PresetConfigSection {
    let mut result = config.to_value();

    for mapping in mappings {
        if let Some(when) = &mapping.when {
            if !evaluate_conditions(when, values) {
                continue;
            }
        }

        let resolved_value = resolve_mapping_value(&mapping.value, values);
        set_value_by_path(&mut result, &mapping.target, resolved_value);
    }

    PresetConfigSection::from_value(result)
}

/// 解析映射 value。
///
/// 当 value 是形如 `#{id}` 的字符串时，
/// 取用户输入的对应字段值。
fn resolve_mapping_value(raw_value: &Value, values: &UserInputValues) -> Value {
    match raw_value {
        Value::String(text) if text.starts_with("#{") && text.ends_with('}') => {
            let key = text.trim_start_matches("#{").trim_end_matches('}');
            values.get(key).cloned().unwrap_or(Value::Null)
        }
        _ => raw_value.clone(),
    }
}

/// 应用用户输入到预设。
pub fn apply_user_inputs(
    preset_file: &PresetFile,
    values: &UserInputValues,
) -> PresetConfigSection {
    let mut config = if let Some(template) = &preset_file.template {
        PresetConfigSection::from_value(replace_template_variables(template, values))
    } else {
        let replaced = replace_template_variables(&preset_file.config.to_value(), values);
        PresetConfigSection::from_value(replaced)
    };

    if let Some(mappings) = &preset_file.config_mappings {
        config = apply_config_mappings(mappings, values, &config);
    }

    // 兼容历史行为：用户输入里的路径写法直接应用。
    let mut as_value = config.to_value();
    for (key, value) in values {
        if key.contains('.') || key.contains('[') {
            set_value_by_path(&mut as_value, key, value.clone());
        }
    }

    PresetConfigSection::from_value(as_value)
}

/// 从 Manifest 加载配置并应用 userValues。
pub fn load_config_from_manifest(
    manifest: &ManifestFile,
    preset_dir: Option<&Path>,
) -> PresetConfigSection {
    let preset_file = manifest_to_preset_file(manifest);

    let mut config = if let Some(values) = &manifest.user_values {
        apply_user_inputs(&preset_file, values)
    } else {
        preset_file.config.clone()
    };

    // 处理 StatusLine 里的相对 scriptPath。
    if let Some(status_line) = &config.status_line {
        config.status_line = Some(process_status_line_config(status_line, preset_dir));
    }

    // 处理 transformers 里的相对 path。
    if let Some(transformers) = &config.transformers {
        config.transformers = Some(process_transformers_config(transformers, preset_dir));
    }

    config
}

/// 将 Manifest 按“元数据/动态字段/配置字段”拆分。
fn manifest_to_preset_file(manifest: &ManifestFile) -> PresetFile {
    let mut config_map = manifest.extra.clone();

    // 这些字段属于动态配置系统，不应进入 config。
    config_map.remove("schema");
    config_map.remove("template");
    config_map.remove("configMappings");
    config_map.remove("userValues");

    let metadata = match (&manifest.name, &manifest.version) {
        (Some(name), Some(version)) => Some(crate::preset::types::PresetMetadata {
            name: name.clone(),
            version: version.clone(),
            description: manifest.description.clone(),
            author: manifest.author.clone(),
            homepage: manifest.homepage.clone(),
            repository: manifest.repository.clone(),
            license: manifest.license.clone(),
            keywords: manifest.keywords.clone(),
            ccr_version: manifest.ccr_version.clone(),
            source: manifest.source.clone(),
            source_type: manifest.source_type.clone(),
            checksum: manifest.checksum.clone(),
        }),
        _ => None,
    };

    PresetFile {
        metadata,
        config: PresetConfigSection::from_value(Value::Object(config_map.into_iter().collect())),
        secrets: None,
        schema: manifest.schema.clone(),
        template: manifest.template.clone(),
        config_mappings: manifest.config_mappings.clone(),
    }
}

/// 将 StatusLine 中的相对脚本路径转为绝对路径。
fn process_status_line_config(status_line: &Value, preset_dir: Option<&Path>) -> Value {
    let Some(root) = preset_dir else {
        return status_line.clone();
    };

    let mut result = status_line.clone();
    let Value::Object(theme_map) = &mut result else {
        return status_line.clone();
    };

    for (_, theme_value) in theme_map.iter_mut() {
        let Value::Object(theme_obj) = theme_value else {
            continue;
        };

        let Some(modules) = theme_obj.get_mut("modules") else {
            continue;
        };
        let Value::Array(list) = modules else {
            continue;
        };

        for module in list {
            let Value::Object(module_obj) = module else {
                continue;
            };

            let Some(Value::String(script_path)) = module_obj.get("scriptPath") else {
                continue;
            };

            if Path::new(script_path).is_relative() {
                let absolute = root.join(script_path).to_string_lossy().to_string();
                module_obj.insert("scriptPath".to_string(), Value::String(absolute));
            }
        }
    }

    result
}

/// 将 transformers 里的相对 path 转为绝对路径。
fn process_transformers_config(
    transformers: &[crate::preset::types::TransformerConfig],
    preset_dir: Option<&Path>,
) -> Vec<crate::preset::types::TransformerConfig> {
    let Some(root) = preset_dir else {
        return transformers.to_vec();
    };

    transformers
        .iter()
        .map(|item| {
            let mut next = item.clone();
            if let Some(path) = &next.path {
                let raw = Path::new(path);
                if raw.is_relative() {
                    next.path = Some(root.join(raw).to_string_lossy().to_string());
                }
            }
            next
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::{apply_user_inputs, replace_template_variables};
    use crate::preset::types::{PresetConfigSection, PresetFile};

    #[test]
    fn test_replace_template_variables() {
        let template = json!({"api_key": "#{apiKey}"});
        let mut values = BTreeMap::new();
        values.insert("apiKey".to_string(), json!("sk-test"));
        assert_eq!(
            replace_template_variables(&template, &values),
            json!({"api_key": "sk-test"})
        );
    }

    #[test]
    fn test_apply_user_inputs_with_template() {
        let preset = PresetFile {
            template: Some(
                json!({"Providers": [{"name": "openai", "api_base_url": "https://api.openai.com/v1/chat/completions", "api_key": "#{apiKey}", "models": ["gpt-5"]}]}),
            ),
            config: PresetConfigSection::default(),
            ..PresetFile::default()
        };

        let mut values = BTreeMap::new();
        values.insert("apiKey".to_string(), json!("sk-test"));

        let config = apply_user_inputs(&preset, &values);
        let value = config.to_value();
        assert_eq!(value["Providers"][0]["api_key"], json!("sk-test"));
    }
}
