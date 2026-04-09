//! 预设导出能力。

use std::path::PathBuf;

use tokio::fs;

use crate::constants::PRESETS_DIR;
use crate::error::SharedError;
use crate::preset::sensitive_fields::sanitize_config;
use crate::preset::types::{ManifestFile, PresetMetadata};

/// 导出参数。
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    pub include_sensitive: bool,
    pub description: Option<String>,
    pub author: Option<String>,
    pub tags: Option<String>,
}

/// 导出结果。
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub preset_dir: PathBuf,
    pub sanitized_config: serde_json::Value,
    pub metadata: PresetMetadata,
    pub sanitized_count: usize,
}

/// 构造 manifest。
pub fn create_manifest(
    preset_name: &str,
    _raw_config: &serde_json::Value,
    sanitized_config: &serde_json::Value,
    options: &ExportOptions,
) -> ManifestFile {
    let extra: std::collections::BTreeMap<String, serde_json::Value> = sanitized_config
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let keywords = options.tags.as_ref().map(|text| {
        text.split(',')
            .map(|item| item.trim().to_string())
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
    });

    ManifestFile {
        name: Some(preset_name.to_string()),
        version: Some("1.0.0".to_string()),
        description: options.description.clone(),
        author: options.author.clone(),
        keywords,
        extra,
        ..ManifestFile::default()
    }
}

/// 导出 preset 到本地目录。
pub async fn export_preset(
    preset_name: &str,
    config: &serde_json::Value,
    options: ExportOptions,
) -> Result<ExportResult, SharedError> {
    let metadata = PresetMetadata {
        name: preset_name.to_string(),
        version: "1.0.0".to_string(),
        description: options.description.clone(),
        author: options.author.clone(),
        keywords: options.tags.as_ref().map(|text| {
            text.split(',')
                .map(|item| item.trim().to_string())
                .filter(|item| !item.is_empty())
                .collect::<Vec<_>>()
        }),
        ..PresetMetadata::default()
    };

    // 是否保留敏感字段由 include_sensitive 决定。
    let (sanitized_config, sanitized_count) = if options.include_sensitive {
        (config.clone(), 0)
    } else {
        let result = sanitize_config(config).await;
        (result.sanitized_config, result.sanitized_count)
    };

    let manifest = create_manifest(preset_name, config, &sanitized_config, &options);
    let preset_dir = PRESETS_DIR.join(preset_name);

    if preset_dir.exists() {
        return Err(SharedError::PresetAlreadyExists(
            preset_dir.to_string_lossy().to_string(),
        ));
    }

    fs::create_dir_all(&preset_dir).await?;
    let content = serde_json::to_string_pretty(&manifest)?;
    fs::write(preset_dir.join("manifest.json"), content).await?;

    Ok(ExportResult {
        preset_dir,
        sanitized_config,
        metadata,
        sanitized_count,
    })
}
