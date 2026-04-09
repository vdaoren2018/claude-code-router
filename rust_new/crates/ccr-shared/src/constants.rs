//! 目录与默认配置常量。
//!
//! 说明：为避免在初始化阶段频繁拼接路径，
//! 这里使用 `LazyLock<PathBuf>` 做一次性计算。

use std::path::PathBuf;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

/// 用户主目录下的 ccr 根目录。
pub static HOME_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".claude-code-router")
});

/// 配置文件路径。
pub static CONFIG_FILE: LazyLock<PathBuf> = LazyLock::new(|| HOME_DIR.join("config.json"));

/// 插件目录路径。
pub static PLUGINS_DIR: LazyLock<PathBuf> = LazyLock::new(|| HOME_DIR.join("plugins"));

/// Preset 目录路径。
pub static PRESETS_DIR: LazyLock<PathBuf> = LazyLock::new(|| HOME_DIR.join("presets"));

/// 进程 PID 文件路径。
pub static PID_FILE: LazyLock<PathBuf> = LazyLock::new(|| HOME_DIR.join(".claude-code-router.pid"));

/// 引用计数文件路径（用于多进程协作）。
pub static REFERENCE_COUNT_FILE: LazyLock<PathBuf> =
    LazyLock::new(|| std::env::temp_dir().join("claude-code-reference-count.txt"));

/// Claude 项目目录（用于 session 反查）。
pub static CLAUDE_PROJECTS_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".claude").join("projects")
});

/// 默认配置结构。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DefaultConfig {
    /// 是否开启日志。
    #[serde(rename = "LOG")]
    pub log: bool,
    /// OpenAI key。
    #[serde(rename = "OPENAI_API_KEY")]
    pub openai_api_key: String,
    /// OpenAI base url。
    #[serde(rename = "OPENAI_BASE_URL")]
    pub openai_base_url: String,
    /// OpenAI model。
    #[serde(rename = "OPENAI_MODEL")]
    pub openai_model: String,
}

impl Default for DefaultConfig {
    fn default() -> Self {
        Self {
            log: false,
            openai_api_key: String::new(),
            openai_base_url: String::new(),
            openai_model: String::new(),
        }
    }
}
