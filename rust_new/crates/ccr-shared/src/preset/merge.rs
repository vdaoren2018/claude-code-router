//! 配置合并策略。

use serde_json::Value;

use crate::preset::types::{MergeStrategy, PresetConfigSection};

/// 合并冲突回调。
pub trait MergeCallbacks {
    /// Router 冲突回调。
    fn on_router_conflict(&self, _key: &str, _existing: &Value, _incoming: &Value) -> bool {
        false
    }

    /// Transformer 冲突回调。
    fn on_transformer_conflict(&self, _path: &str) -> &'static str {
        "keep"
    }

    /// 顶层配置冲突回调。
    fn on_config_conflict(&self, _key: &str) -> bool {
        false
    }
}

/// 合并配置。
///
/// 规则：
/// - overwrite：冲突时覆盖。
/// - merge/skip：冲突时保留旧值。
/// - ask：通过回调决定是否覆盖。
pub async fn merge_config(
    base_config: &PresetConfigSection,
    preset_config: &PresetConfigSection,
    strategy: MergeStrategy,
    callbacks: Option<&dyn MergeCallbacks>,
) -> PresetConfigSection {
    let mut base = base_config.to_value();
    let incoming = preset_config.to_value();

    merge_value(&mut base, &incoming, strategy, callbacks);

    PresetConfigSection::from_value(base)
}

/// 对 JSON Object 做顶层递归合并。
fn merge_value(
    target: &mut Value,
    incoming: &Value,
    strategy: MergeStrategy,
    callbacks: Option<&dyn MergeCallbacks>,
) {
    let (Value::Object(target_map), Value::Object(incoming_map)) = (target, incoming) else {
        return;
    };

    for (key, incoming_value) in incoming_map {
        match target_map.get_mut(key) {
            Some(existing_value) => {
                if existing_value == incoming_value {
                    continue;
                }

                let should_overwrite = match strategy {
                    MergeStrategy::Overwrite => true,
                    MergeStrategy::Merge | MergeStrategy::Skip => false,
                    MergeStrategy::Ask => {
                        if key == "Router" {
                            callbacks
                                .map(|cb| {
                                    cb.on_router_conflict(key, existing_value, incoming_value)
                                })
                                .unwrap_or(false)
                        } else {
                            callbacks
                                .map(|cb| cb.on_config_conflict(key))
                                .unwrap_or(false)
                        }
                    }
                };

                if should_overwrite {
                    *existing_value = incoming_value.clone();
                }
            }
            None => {
                target_map.insert(key.clone(), incoming_value.clone());
            }
        }
    }
}
