//! ConfigService 实现。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::ConfigError;
use crate::interpolate::interpolate_env_vars;

/// AppConfig 在 Rust 里用 JSON 对象承载。
pub type AppConfig = Value;

/// 配置服务初始化参数。
#[derive(Debug, Clone)]
pub struct ConfigOptions {
    pub env_path: Option<PathBuf>,
    pub json_path: Option<PathBuf>,
    pub use_env_file: bool,
    pub use_json_file: bool,
    pub use_environment_variables: bool,
    pub initial_config: Option<Value>,
}

impl Default for ConfigOptions {
    fn default() -> Self {
        Self {
            env_path: Some(PathBuf::from(".env")),
            json_path: Some(PathBuf::from("./config.json")),
            use_env_file: false,
            use_json_file: true,
            use_environment_variables: true,
            initial_config: None,
        }
    }
}

/// 配置服务。
#[derive(Debug, Clone)]
pub struct ConfigService {
    config: Value,
    options: ConfigOptions,
}

impl ConfigService {
    /// 创建配置服务并加载配置。
    pub fn new(options: ConfigOptions) -> Result<Self, ConfigError> {
        let mut this = Self {
            config: Value::Object(Map::new()),
            options,
        };
        this.load_config()?;
        Ok(this)
    }

    /// 重新加载配置。
    pub fn reload(&mut self) -> Result<(), ConfigError> {
        self.config = Value::Object(Map::new());
        self.load_config()
    }

    /// 获取单个配置值（反序列化）。
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.config
            .as_object()
            .and_then(|map| map.get(key))
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// 获取配置值，如果缺失则返回默认值。
    pub fn get_or<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        self.get(key).unwrap_or(default)
    }

    /// 获取原始配置。
    pub fn get_all(&self) -> Value {
        self.config.clone()
    }

    /// 是否存在某个字段。
    pub fn has(&self, key: &str) -> bool {
        self.config
            .as_object()
            .map(|map| map.contains_key(key))
            .unwrap_or(false)
    }

    /// 设置字段值。
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        if let Some(map) = self.config.as_object_mut() {
            map.insert(key.into(), value);
        }
    }

    /// 获取 HTTPS 代理配置。
    pub fn get_https_proxy(&self) -> Option<String> {
        self.get::<String>("HTTPS_PROXY")
            .or_else(|| self.get("https_proxy"))
            .or_else(|| self.get("httpsProxy"))
            .or_else(|| self.get("PROXY_URL"))
    }

    /// 返回配置来源摘要。
    pub fn get_config_summary(&self) -> String {
        let mut summary = vec![];

        if self.options.initial_config.is_some() {
            summary.push("Initial Config".to_string());
        }
        if self.options.use_json_file {
            summary.push(format!(
                "JSON: {}",
                self.options
                    .json_path
                    .as_ref()
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ));
        }
        if self.options.use_env_file {
            summary.push(format!(
                "ENV: {}",
                self.options
                    .env_path
                    .as_ref()
                    .map(|v| v.to_string_lossy().to_string())
                    .unwrap_or_else(|| "<none>".to_string())
            ));
        }
        if self.options.use_environment_variables {
            summary.push("Environment Variables".to_string());
        }

        format!("Config sources: {}", summary.join(", "))
    }

    /// 内部加载流程：json -> initial -> env file -> env vars。
    fn load_config(&mut self) -> Result<(), ConfigError> {
        if self.options.use_json_file {
            if let Some(path) = self.options.json_path.clone() {
                self.load_json_config(&path)?;
            }
        }

        if let Some(initial) = self.options.initial_config.clone() {
            merge_top_level(&mut self.config, &initial);
        }

        if self.options.use_env_file {
            if let Some(path) = self.options.env_path.clone() {
                self.load_env_config(&path)?;
            }
        }

        if self.options.use_environment_variables {
            self.load_environment_variables();
        }

        normalize_compat_fields(&mut self.config);
        self.config = interpolate_env_vars(&self.config);
        Ok(())
    }

    /// 读取 JSON5 配置文件。
    fn load_json_config(&mut self, path: &Path) -> Result<(), ConfigError> {
        let path = normalize_path(path);
        if !path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&path)?;
        let parsed =
            json5::from_str::<Value>(&content).map_err(|e| ConfigError::Json5(e.to_string()))?;
        merge_top_level(&mut self.config, &parsed);
        Ok(())
    }

    /// 读取 `.env` 文件并浅层写入。
    fn load_env_config(&mut self, path: &Path) -> Result<(), ConfigError> {
        let path = normalize_path(path);
        if !path.exists() {
            return Ok(());
        }

        let iter = dotenvy::from_path_iter(path)
            .map_err(|err| ConfigError::InvalidConfig(format!("读取 env 文件失败: {err}")))?;

        let mut env_map = Map::new();
        for pair in iter {
            let (k, v) = pair
                .map_err(|err| ConfigError::InvalidConfig(format!("解析 env 键值失败: {err}")))?;
            env_map.insert(k, Value::String(v));
        }

        merge_top_level(&mut self.config, &Value::Object(env_map));
        Ok(())
    }

    /// 将当前进程环境变量并入配置（仅字符串）。
    fn load_environment_variables(&mut self) {
        let mut env_map = BTreeMap::new();
        for (k, v) in std::env::vars() {
            env_map.insert(k, Value::String(v));
        }

        let value = Value::Object(env_map.into_iter().collect());
        merge_top_level(&mut self.config, &value);
    }
}

/// 路径归一化：相对路径转绝对路径。
fn normalize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

/// 顶层浅合并（对齐 TS `...spread` 行为）。
fn merge_top_level(target: &mut Value, incoming: &Value) {
    let Value::Object(target_map) = target else {
        *target = Value::Object(Map::new());
        merge_top_level(target, incoming);
        return;
    };

    let Value::Object(incoming_map) = incoming else {
        return;
    };

    for (key, value) in incoming_map {
        target_map.insert(key.clone(), value.clone());
    }
}

/// 兼容字段归一化。
///
/// - `Providers` -> `providers`
/// - `providers` -> `Providers`
/// - `Plugins` <-> `plugins`
fn normalize_compat_fields(config: &mut Value) {
    let Some(map) = config.as_object_mut() else {
        return;
    };

    if map.contains_key("Providers") && !map.contains_key("providers") {
        if let Some(value) = map.get("Providers").cloned() {
            map.insert("providers".to_string(), value);
        }
    }

    if map.contains_key("providers") && !map.contains_key("Providers") {
        if let Some(value) = map.get("providers").cloned() {
            map.insert("Providers".to_string(), value);
        }
    }

    if map.contains_key("Plugins") && !map.contains_key("plugins") {
        if let Some(value) = map.get("Plugins").cloned() {
            map.insert("plugins".to_string(), value);
        }
    }

    if map.contains_key("plugins") && !map.contains_key("Plugins") {
        if let Some(value) = map.get("plugins").cloned() {
            map.insert("Plugins".to_string(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use tempfile::tempdir;

    use super::{ConfigOptions, ConfigService};

    #[test]
    fn test_load_json5_and_alias() {
        let dir = tempdir().expect("create temp dir");
        let config_path = dir.path().join("config.json");
        std::fs::write(
            &config_path,
            r#"{ Providers: [{ name: 'openai', api_base_url: 'x', api_key: 'y', models: ['gpt-5'] }] }"#,
        )
        .expect("write config");

        let options = ConfigOptions {
            json_path: Some(config_path),
            ..ConfigOptions::default()
        };

        let service = ConfigService::new(options).expect("load config");
        let providers: Option<Vec<serde_json::Value>> = service.get("providers");
        assert!(providers.is_some());
    }

    #[test]
    fn test_initial_config_merge() {
        let options = ConfigOptions {
            use_json_file: false,
            use_environment_variables: false,
            initial_config: Some(json!({"PORT": 3456})),
            ..ConfigOptions::default()
        };

        let service = ConfigService::new(options).expect("new");
        let port: Option<u16> = service.get("PORT");
        assert_eq!(port, Some(3456));
    }

    #[test]
    fn test_get_https_proxy() {
        let options = ConfigOptions {
            use_json_file: false,
            use_environment_variables: false,
            initial_config: Some(json!({"PROXY_URL": "http://127.0.0.1:7890"})),
            ..ConfigOptions::default()
        };
        let service = ConfigService::new(options).expect("new");
        assert_eq!(
            service.get_https_proxy(),
            Some("http://127.0.0.1:7890".to_string())
        );
    }

    #[test]
    fn test_summary() {
        let options = ConfigOptions {
            env_path: Some(PathBuf::from(".env.test")),
            use_env_file: true,
            use_environment_variables: false,
            use_json_file: false,
            ..ConfigOptions::default()
        };
        let service = ConfigService::new(options).expect("new");
        assert!(service.get_config_summary().contains("ENV"));
    }
}
