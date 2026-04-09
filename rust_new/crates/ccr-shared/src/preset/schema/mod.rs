//! Schema 处理入口。

mod apply;
mod conditions;
mod dependency;
mod options;
mod path_utils;

pub use apply::{
    apply_config_mappings, apply_user_inputs, load_config_from_manifest, replace_template_variables,
};
pub use conditions::{evaluate_condition, evaluate_conditions, should_show_field};
pub use dependency::{
    InputValidationResult, build_dependency_graph, get_affected_fields, get_default_value,
    sort_fields_by_dependencies, validate_input,
};
pub use options::{get_dynamic_options, resolve_options};
pub use path_utils::{get_value_by_path, parse_field_path, set_value_by_path};
