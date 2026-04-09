//! 条件表达式计算。

use serde_json::Value;

use crate::preset::types::{
    Condition, ConditionOperator, FieldWhen, RequiredInput, UserInputValues,
};

/// 读取用户输入里的字段值。
fn get_actual_value<'a>(values: &'a UserInputValues, field: &str) -> Option<&'a Value> {
    values.get(field)
}

/// 计算单个条件。
pub fn evaluate_condition(condition: &Condition, values: &UserInputValues) -> bool {
    let actual_value = get_actual_value(values, &condition.field);
    let operator = condition.operator.clone().unwrap_or(ConditionOperator::Eq);

    match operator {
        ConditionOperator::Exists => actual_value.is_some_and(|v| !v.is_null()),
        ConditionOperator::In => {
            let Some(actual) = actual_value else {
                return false;
            };
            let Some(Value::Array(expected)) = condition.value.clone() else {
                return false;
            };
            expected.iter().any(|item| item == actual)
        }
        ConditionOperator::Nin => {
            let Some(actual) = actual_value else {
                return false;
            };
            let Some(Value::Array(expected)) = condition.value.clone() else {
                return false;
            };
            !expected.iter().any(|item| item == actual)
        }
        ConditionOperator::Eq => actual_value == condition.value.as_ref(),
        ConditionOperator::Ne => actual_value != condition.value.as_ref(),
        ConditionOperator::Gt => {
            compare_number(actual_value, condition.value.as_ref(), |a, b| a > b)
        }
        ConditionOperator::Lt => {
            compare_number(actual_value, condition.value.as_ref(), |a, b| a < b)
        }
        ConditionOperator::Gte => {
            compare_number(actual_value, condition.value.as_ref(), |a, b| a >= b)
        }
        ConditionOperator::Lte => {
            compare_number(actual_value, condition.value.as_ref(), |a, b| a <= b)
        }
    }
}

/// 数值比较小工具。
fn compare_number<F>(actual: Option<&Value>, expected: Option<&Value>, cmp: F) -> bool
where
    F: Fn(f64, f64) -> bool,
{
    let Some(actual) = actual.and_then(Value::as_f64) else {
        return false;
    };
    let Some(expected) = expected.and_then(Value::as_f64) else {
        return false;
    };
    cmp(actual, expected)
}

/// 计算多条件（AND 语义）。
pub fn evaluate_conditions(when: &FieldWhen, values: &UserInputValues) -> bool {
    match when {
        FieldWhen::Single(condition) => evaluate_condition(condition, values),
        FieldWhen::Multi(list) => list
            .iter()
            .all(|condition| evaluate_condition(condition, values)),
    }
}

/// 判断字段是否可见。
pub fn should_show_field(field: &RequiredInput, values: &UserInputValues) -> bool {
    match &field.when {
        Some(when) => evaluate_conditions(when, values),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::evaluate_condition;
    use crate::preset::types::{Condition, ConditionOperator};

    #[test]
    fn test_condition_eq() {
        let mut values = BTreeMap::new();
        values.insert("provider".to_string(), json!("openai"));
        let condition = Condition {
            field: "provider".to_string(),
            operator: Some(ConditionOperator::Eq),
            value: Some(json!("openai")),
        };
        assert!(evaluate_condition(&condition, &values));
    }
}
