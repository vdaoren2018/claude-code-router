//! 共享基础库。
//!
//! 该 crate 负责承载 Rust 重构中的公共能力，
//! 包含目录常量、Preset 系统和兼容工具函数。

pub mod constants;
pub mod error;
pub mod preset;

pub use constants::{
    CLAUDE_PROJECTS_DIR, CONFIG_FILE, HOME_DIR, PID_FILE, PLUGINS_DIR, PRESETS_DIR,
    REFERENCE_COUNT_FILE,
};

// 兼容导出：后续业务层可直接使用这些常用函数。
pub use preset::export::{ExportOptions, ExportResult, create_manifest, export_preset};
pub use preset::install::{
    download_preset_to_temp, extract_metadata, extract_preset, get_preset_dir, get_temp_dir,
    is_preset_installed, list_presets, load_preset, manifest_to_preset_file,
    read_manifest_from_dir, save_manifest, validate_preset,
};
pub use preset::marketplace::{find_market_preset_by_name, get_market_presets};
pub use preset::merge::{MergeCallbacks, merge_config};
pub use preset::read_preset::read_preset_file;
pub use preset::schema::{
    InputValidationResult, apply_config_mappings, apply_user_inputs, build_dependency_graph,
    evaluate_condition, evaluate_conditions, get_affected_fields, get_default_value,
    get_dynamic_options, get_value_by_path, load_config_from_manifest, parse_field_path,
    replace_template_variables, resolve_options, set_value_by_path, should_show_field,
    sort_fields_by_dependencies, validate_input,
};
pub use preset::sensitive_fields::{
    extract_env_var_name, fill_sensitive_inputs, generate_env_var_name, sanitize_config,
};
pub use preset::types::*;
