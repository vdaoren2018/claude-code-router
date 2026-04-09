//! 输入校验与依赖计算。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use crate::preset::types::{FieldOptions, InputType, RequiredInput};

/// 输入校验结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputValidationResult {
    pub valid: bool,
    pub error: Option<String>,
}

/// 校验输入值。
pub fn validate_input(field: &RequiredInput, value: &Value) -> InputValidationResult {
    // required 默认 true。
    if field.required.unwrap_or(true) {
        let empty =
            matches!(value, Value::Null) || matches!(value, Value::String(text) if text.is_empty());
        if empty {
            return InputValidationResult {
                valid: false,
                error: Some(format!(
                    "{} 为必填项",
                    field.label.clone().unwrap_or(field.id.clone())
                )),
            };
        }
    }

    match field.input_type.clone().unwrap_or(InputType::Password) {
        InputType::Number => {
            let Some(number) = value.as_f64() else {
                return InputValidationResult {
                    valid: false,
                    error: Some(format!("{} 必须是数值", field.id)),
                };
            };

            if let Some(min) = field.min {
                if number < min {
                    return InputValidationResult {
                        valid: false,
                        error: Some(format!("{} 不能小于 {min}", field.id)),
                    };
                }
            }

            if let Some(max) = field.max {
                if number > max {
                    return InputValidationResult {
                        valid: false,
                        error: Some(format!("{} 不能大于 {max}", field.id)),
                    };
                }
            }
        }
        InputType::Select => {
            if let Some(FieldOptions::Static(options)) = &field.options {
                let exists = options.iter().any(|option| &option.value == value);
                if !exists {
                    return InputValidationResult {
                        valid: false,
                        error: Some(format!("{} 的选项不合法", field.id)),
                    };
                }
            }
        }
        _ => {}
    }

    InputValidationResult {
        valid: true,
        error: None,
    }
}

/// 获取字段默认值。
pub fn get_default_value(field: &RequiredInput) -> Value {
    if let Some(value) = &field.default_value {
        return value.clone();
    }

    match field.input_type.clone().unwrap_or(InputType::Password) {
        InputType::Confirm => Value::Bool(false),
        InputType::Multiselect => Value::Array(vec![]),
        InputType::Number => Value::from(0),
        _ => Value::String(String::new()),
    }
}

/// 按依赖关系排序字段（简单 DFS 拓扑）。
pub fn sort_fields_by_dependencies(fields: &[RequiredInput]) -> Vec<RequiredInput> {
    let mut sorted = Vec::new();
    let mut visited = HashSet::new();
    let field_map: HashMap<String, RequiredInput> = fields
        .iter()
        .cloned()
        .map(|field| (field.id.clone(), field))
        .collect();

    fn visit(
        id: &str,
        field_map: &HashMap<String, RequiredInput>,
        visited: &mut HashSet<String>,
        sorted: &mut Vec<RequiredInput>,
    ) {
        if visited.contains(id) {
            return;
        }
        visited.insert(id.to_string());

        let Some(field) = field_map.get(id) else {
            return;
        };

        if let Some(depends_on) = &field.depends_on {
            for dep in depends_on {
                visit(dep, field_map, visited, sorted);
            }
        }

        sorted.push(field.clone());
    }

    for field in fields {
        visit(&field.id, &field_map, &mut visited, &mut sorted);
    }

    sorted
}

/// 构建“字段 -> 依赖字段”图。
pub fn build_dependency_graph(fields: &[RequiredInput]) -> HashMap<String, HashSet<String>> {
    let mut graph = HashMap::new();

    for field in fields {
        let mut deps = HashSet::new();

        if let Some(depends_on) = &field.depends_on {
            deps.extend(depends_on.iter().cloned());
        }

        if let Some(when) = &field.when {
            match when {
                crate::preset::types::FieldWhen::Single(cond) => {
                    deps.insert(cond.field.clone());
                }
                crate::preset::types::FieldWhen::Multi(list) => {
                    for cond in list {
                        deps.insert(cond.field.clone());
                    }
                }
            }
        }

        if let Some(FieldOptions::Dynamic(dynamic)) = &field.options {
            if let Some(provider_field) = &dynamic.provider_field {
                let provider_key = provider_field
                    .trim_start_matches("#{")
                    .trim_end_matches('}')
                    .to_string();
                deps.insert(provider_key);
            }
        }

        graph.insert(field.id.clone(), deps);
    }

    graph
}

/// 获取受变更字段影响的字段集合。
pub fn get_affected_fields(changed_field_id: &str, fields: &[RequiredInput]) -> HashSet<String> {
    let graph = build_dependency_graph(fields);
    graph
        .into_iter()
        .filter_map(|(field_id, deps)| deps.contains(changed_field_id).then_some(field_id))
        .collect()
}
