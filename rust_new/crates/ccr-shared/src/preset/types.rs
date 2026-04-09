//! Preset 类型定义。
//!
//! 该文件对齐了现有 TypeScript 的字段语义，
//! 同时尽量保留 Rust 的类型安全优势。

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 用户输入值字典。
pub type UserInputValues = BTreeMap<String, Value>;

/// 输入控件类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    /// 密码框。
    Password,
    /// 单行输入。
    Input,
    /// 单选。
    Select,
    /// 多选。
    Multiselect,
    /// 布尔确认。
    Confirm,
    /// 多行编辑。
    Editor,
    /// 数值输入。
    Number,
}

/// 条件运算符。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConditionOperator {
    Eq,
    Ne,
    In,
    Nin,
    Gt,
    Lt,
    Gte,
    Lte,
    Exists,
}

/// 选项项定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InputOption {
    pub label: String,
    pub value: Value,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub icon: Option<String>,
}

/// 动态选项来源类型。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DynamicOptionsType {
    Static,
    Providers,
    Models,
    Custom,
}

/// 动态选项定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DynamicOptions {
    #[serde(rename = "type")]
    pub source_type: DynamicOptionsType,
    #[serde(default)]
    pub options: Option<Vec<InputOption>>,
    #[serde(default, rename = "providerField")]
    pub provider_field: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

/// 字段 options 可以是静态数组，也可以是动态来源。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldOptions {
    /// 直接写死的 options。
    Static(Vec<InputOption>),
    /// 动态计算 options。
    Dynamic(DynamicOptions),
}

/// 条件表达式。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub field: String,
    #[serde(default)]
    pub operator: Option<ConditionOperator>,
    #[serde(default)]
    pub value: Option<Value>,
}

/// 字段显示条件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum FieldWhen {
    Single(Condition),
    Multi(Vec<Condition>),
}

/// 输入字段定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequiredInput {
    pub id: String,
    #[serde(default, rename = "type")]
    pub input_type: Option<InputType>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub prompt: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub options: Option<FieldOptions>,
    #[serde(default)]
    pub when: Option<FieldWhen>,
    #[serde(default, rename = "defaultValue")]
    pub default_value: Option<Value>,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub validator: Option<Value>,
    #[serde(default)]
    pub min: Option<f64>,
    #[serde(default)]
    pub max: Option<f64>,
    #[serde(default)]
    pub rows: Option<u32>,
    #[serde(default, rename = "dependsOn")]
    pub depends_on: Option<Vec<String>>,
}

/// Provider 配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub name: String,
    pub api_base_url: String,
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub transformer: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Router 配置。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RouterConfig {
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub think: Option<String>,
    #[serde(default, rename = "longContext")]
    pub long_context: Option<String>,
    #[serde(default, rename = "longContextThreshold")]
    pub long_context_threshold: Option<u64>,
    #[serde(default, rename = "webSearch")]
    pub web_search: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Transformer 配置。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransformerConfig {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default, rename = "use")]
    pub use_chain: Vec<Value>,
    #[serde(default)]
    pub options: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Preset 元数据。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresetMetadata {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default, rename = "ccrVersion")]
    pub ccr_version: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "sourceType")]
    pub source_type: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
}

/// Preset 配置区域。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresetConfigSection {
    #[serde(default, rename = "Providers")]
    pub providers: Option<Vec<ProviderConfig>>,
    #[serde(default, rename = "Router")]
    pub router: Option<RouterConfig>,
    #[serde(default)]
    pub transformers: Option<Vec<TransformerConfig>>,
    #[serde(default, rename = "StatusLine")]
    pub status_line: Option<Value>,
    #[serde(default, rename = "NON_INTERACTIVE_MODE")]
    pub non_interactive_mode: Option<bool>,
    #[serde(default, rename = "noServer")]
    pub no_server: Option<bool>,
    #[serde(default, rename = "claudeCodeSettings")]
    pub claude_code_settings: Option<Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl PresetConfigSection {
    /// 转为 JSON 对象，便于做动态路径写入。
    pub fn to_value(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Object(Default::default()))
    }

    /// 从 JSON 对象恢复配置。
    pub fn from_value(value: Value) -> Self {
        serde_json::from_value(value).unwrap_or_default()
    }
}

/// 模板配置。
pub type TemplateConfig = Value;

/// 配置映射定义。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConfigMapping {
    pub target: String,
    pub value: Value,
    #[serde(default)]
    pub when: Option<FieldWhen>,
}

/// Preset 主文件。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresetFile {
    #[serde(default)]
    pub metadata: Option<PresetMetadata>,
    #[serde(default)]
    pub config: PresetConfigSection,
    #[serde(default)]
    pub secrets: Option<HashMap<String, String>>,
    #[serde(default)]
    pub schema: Option<Vec<RequiredInput>>,
    #[serde(default)]
    pub template: Option<TemplateConfig>,
    #[serde(default, rename = "configMappings")]
    pub config_mappings: Option<Vec<ConfigMapping>>,
}

/// Manifest 文件结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ManifestFile {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub repository: Option<String>,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    #[serde(default, rename = "ccrVersion")]
    pub ccr_version: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "sourceType")]
    pub source_type: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub schema: Option<Vec<RequiredInput>>,
    #[serde(default)]
    pub template: Option<TemplateConfig>,
    #[serde(default, rename = "configMappings")]
    pub config_mappings: Option<Vec<ConfigMapping>>,
    #[serde(default, rename = "userValues")]
    pub user_values: Option<UserInputValues>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 市场索引项。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetIndexEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 预设注册表结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PresetRegistry {
    #[serde(default)]
    pub presets: Vec<PresetIndexEntry>,
}

/// 校验结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct ValidationResult {
    pub valid: bool,
    #[serde(default)]
    pub errors: Vec<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}

/// 脱敏结果。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct SanitizeResult {
    #[serde(rename = "sanitizedConfig")]
    pub sanitized_config: Value,
    #[serde(rename = "sanitizedCount")]
    pub sanitized_count: usize,
}

/// 预设信息（用于列表展示）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PresetInfo {
    pub name: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    pub config: PresetConfigSection,
}

/// 配置合并策略。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MergeStrategy {
    /// 交互询问。
    Ask,
    /// 强制覆盖。
    Overwrite,
    /// 智能合并。
    Merge,
    /// 跳过冲突项。
    Skip,
}
