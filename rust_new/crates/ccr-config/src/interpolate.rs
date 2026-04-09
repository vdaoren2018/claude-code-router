//! 环境变量插值工具。

use regex::Regex;
use serde_json::{Map, Value};

/// 对配置值做递归环境变量替换。
pub fn interpolate_env_vars(value: &Value) -> Value {
    match value {
        Value::String(raw) => Value::String(replace_in_string(raw)),
        Value::Array(list) => Value::Array(list.iter().map(interpolate_env_vars).collect()),
        Value::Object(map) => {
            let mut result = Map::new();
            for (key, child) in map {
                result.insert(key.clone(), interpolate_env_vars(child));
            }
            Value::Object(result)
        }
        _ => value.clone(),
    }
}

/// 替换字符串中的 `$VAR` / `${VAR}`。
fn replace_in_string(input: &str) -> String {
    let regex =
        Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)\}|\$([A-Z_][A-Z0-9_]*)").expect("插值 regex 构建失败");

    regex
        .replace_all(input, |caps: &regex::Captures<'_>| {
            let key = caps
                .get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str())
                .unwrap_or_default();
            std::env::var(key).unwrap_or_else(|_| caps[0].to_string())
        })
        .to_string()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::interpolate_env_vars;

    #[test]
    fn test_interpolate_env_vars() {
        unsafe {
            std::env::set_var("CCR_KEY", "abc");
        }
        let raw = json!({"a": "$CCR_KEY", "b": "${CCR_KEY}"});
        let got = interpolate_env_vars(&raw);
        assert_eq!(got["a"], json!("abc"));
        assert_eq!(got["b"], json!("abc"));
    }
}
