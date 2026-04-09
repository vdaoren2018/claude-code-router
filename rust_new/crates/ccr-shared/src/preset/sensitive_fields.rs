//! 敏感字段识别与脱敏能力。

use regex::Regex;
use serde_json::{Map, Value};

use crate::preset::types::SanitizeResult;

/// 敏感字段关键词。
const SENSITIVE_PATTERNS: &[&str] = &[
    "api_key",
    "apikey",
    "secret",
    "token",
    "password",
    "private_key",
    "access_key",
];

/// 环境变量占位符规则。
fn env_regex() -> Regex {
    Regex::new(r"^\$\{?[A-Z_][A-Z0-9_]*\}?$").expect("env regex 构建失败")
}

/// 判断字段名是否敏感。
fn is_sensitive_field(field_name: &str) -> bool {
    let lower = field_name.to_lowercase();
    SENSITIVE_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

/// 生成环境变量名。
pub fn generate_env_var_name(field_type: &str, entity_name: &str, field_name: &str) -> String {
    let _ = field_type;
    let prefix = entity_name
        .to_uppercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();
    let field = field_name
        .to_uppercase()
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
        .collect::<String>();

    if prefix == field {
        return prefix;
    }
    format!("{prefix}_{field}")
}

/// 判断是否是环境变量占位符。
fn is_env_placeholder(value: &str) -> bool {
    env_regex().is_match(value.trim())
}

/// 提取环境变量名。
pub fn extract_env_var_name(value: &str) -> Option<String> {
    let text = value.trim();

    if let Some(raw) = text.strip_prefix("${").and_then(|v| v.strip_suffix('}')) {
        return Some(raw.to_string());
    }

    text.strip_prefix('$').map(|raw| raw.to_string())
}

/// 对外暴露的脱敏函数。
pub async fn sanitize_config(config: &Value) -> SanitizeResult {
    let (sanitized, count) = sanitize_value(config, "CONFIG");
    SanitizeResult {
        sanitized_config: sanitized,
        sanitized_count: count,
    }
}

/// 递归处理 Value，返回“脱敏后值 + 脱敏计数”。
fn sanitize_value(value: &Value, entity_name: &str) -> (Value, usize) {
    match value {
        Value::Object(map) => sanitize_object(map, entity_name),
        Value::Array(list) => {
            let mut count = 0usize;
            let mut result = Vec::with_capacity(list.len());
            for item in list {
                let (sanitized, next_count) = sanitize_value(item, entity_name);
                result.push(sanitized);
                count += next_count;
            }
            (Value::Array(result), count)
        }
        _ => (value.clone(), 0),
    }
}

/// 对象级脱敏。
fn sanitize_object(map: &Map<String, Value>, entity_name: &str) -> (Value, usize) {
    let mut count = 0usize;
    let mut result = Map::new();

    for (key, value) in map {
        if is_sensitive_field(key) {
            match value {
                Value::String(raw) if !is_env_placeholder(raw) => {
                    let env_name = generate_env_var_name("global", entity_name, key);
                    result.insert(key.clone(), Value::String(format!("${{{env_name}}}")));
                    count += 1;
                }
                _ => {
                    result.insert(key.clone(), value.clone());
                }
            }
            continue;
        }

        let (sanitized, next_count) = sanitize_value(value, entity_name);
        result.insert(key.clone(), sanitized);
        count += next_count;
    }

    (Value::Object(result), count)
}

/// 将用户输入回填到配置。
pub fn fill_sensitive_inputs(
    config: &Value,
    inputs: &std::collections::HashMap<String, String>,
) -> Value {
    fn walk(
        value: &Value,
        path: &str,
        inputs: &std::collections::HashMap<String, String>,
    ) -> Value {
        match value {
            Value::Object(map) => {
                let mut result = Map::new();
                for (key, child) in map {
                    let next_path = if path.is_empty() {
                        key.to_string()
                    } else {
                        format!("{path}.{key}")
                    };
                    result.insert(key.clone(), walk(child, &next_path, inputs));
                }
                Value::Object(result)
            }
            Value::Array(list) => Value::Array(
                list.iter()
                    .enumerate()
                    .map(|(index, item)| {
                        let next_path = if path.is_empty() {
                            format!("[{index}]")
                        } else {
                            format!("{path}[{index}]")
                        };
                        walk(item, &next_path, inputs)
                    })
                    .collect(),
            ),
            Value::String(text) if is_env_placeholder(text) => inputs
                .get(path)
                .map(|v| Value::String(v.clone()))
                .unwrap_or_else(|| value.clone()),
            _ => value.clone(),
        }
    }

    walk(config, "", inputs)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{extract_env_var_name, sanitize_config};

    #[tokio::test]
    async fn test_sanitize_config() {
        let config = json!({"api_key": "sk-123", "normal": "ok"});
        let result = sanitize_config(&config).await;
        assert_eq!(result.sanitized_count, 1);
        assert!(
            result.sanitized_config["api_key"]
                .as_str()
                .unwrap_or_default()
                .starts_with("${")
        );
    }

    #[test]
    fn test_extract_env_var_name() {
        assert_eq!(
            extract_env_var_name("${OPENAI_API_KEY}"),
            Some("OPENAI_API_KEY".to_string())
        );
        assert_eq!(
            extract_env_var_name("$OPENAI_API_KEY"),
            Some("OPENAI_API_KEY".to_string())
        );
    }
}
