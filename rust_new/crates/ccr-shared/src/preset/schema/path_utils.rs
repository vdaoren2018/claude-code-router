//! 字段路径解析与访问工具。
//!
//! 支持 `Providers[0].api_key` 风格路径。

use serde_json::{Map, Value};

/// 将字段路径拆分为路径段。
///
/// 示例：`Providers[0].name` -> `["Providers", "0", "name"]`。
pub fn parse_field_path(path: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut token = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '.' if !in_bracket => {
                if !token.is_empty() {
                    parts.push(token.clone());
                    token.clear();
                }
            }
            '[' => {
                if !token.is_empty() {
                    parts.push(token.clone());
                    token.clear();
                }
                in_bracket = true;
            }
            ']' => {
                if !token.is_empty() {
                    parts.push(token.clone());
                    token.clear();
                }
                in_bracket = false;
            }
            _ => token.push(ch),
        }
    }

    if !token.is_empty() {
        parts.push(token);
    }

    parts
}

/// 根据路径读取值。
pub fn get_value_by_path<'a>(obj: &'a Value, path: &str) -> Option<&'a Value> {
    let parts = parse_field_path(path);
    let mut current = obj;

    for part in parts {
        match current {
            Value::Object(map) => {
                current = map.get(&part)?;
            }
            Value::Array(list) => {
                let index = part.parse::<usize>().ok()?;
                current = list.get(index)?;
            }
            _ => return None,
        }
    }

    Some(current)
}

/// 根据路径写入值。
///
/// 规则：
/// - 访问对象时，自动补空对象。
/// - 访问数组时，自动扩容并补 null。
pub fn set_value_by_path(obj: &mut Value, path: &str, value: Value) {
    let parts = parse_field_path(path);
    if parts.is_empty() {
        return;
    }

    let mut current = obj;

    for (idx, part) in parts.iter().enumerate() {
        let is_last = idx == parts.len() - 1;

        match current {
            Value::Object(map) => {
                if is_last {
                    map.insert(part.clone(), value);
                    return;
                }

                let next_is_index = parts
                    .get(idx + 1)
                    .and_then(|s| s.parse::<usize>().ok())
                    .is_some();

                let entry = map.entry(part.clone()).or_insert_with(|| {
                    if next_is_index {
                        Value::Array(vec![])
                    } else {
                        Value::Object(Map::new())
                    }
                });

                current = entry;
            }
            Value::Array(list) => {
                let index = match part.parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => return,
                };

                if list.len() <= index {
                    list.resize(index + 1, Value::Null);
                }

                if is_last {
                    list[index] = value;
                    return;
                }

                if list[index].is_null() {
                    let next_is_index = parts
                        .get(idx + 1)
                        .and_then(|s| s.parse::<usize>().ok())
                        .is_some();
                    list[index] = if next_is_index {
                        Value::Array(vec![])
                    } else {
                        Value::Object(Map::new())
                    };
                }

                current = &mut list[index];
            }
            _ => return,
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{get_value_by_path, parse_field_path, set_value_by_path};

    #[test]
    fn test_parse_field_path() {
        let parts = parse_field_path("Providers[0].api_key");
        assert_eq!(parts, vec!["Providers", "0", "api_key"]);
    }

    #[test]
    fn test_set_and_get_path() {
        let mut value = json!({});
        set_value_by_path(&mut value, "Providers[0].api_key", json!("sk-test"));
        let got = get_value_by_path(&value, "Providers[0].api_key");
        assert_eq!(got, Some(&json!("sk-test")));
    }
}
