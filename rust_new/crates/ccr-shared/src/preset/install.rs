//! Preset 安装与读取能力。

use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::fs;
use zip::ZipArchive;

use crate::constants::{HOME_DIR, PRESETS_DIR};
use crate::error::{SharedError, json5_err};
use crate::preset::schema::load_config_from_manifest;
use crate::preset::types::{
    ManifestFile, PresetFile, PresetInfo, PresetMetadata, ValidationResult,
};

/// 元数据字段清单。
const METADATA_FIELDS: &[&str] = &[
    "name",
    "version",
    "description",
    "author",
    "homepage",
    "repository",
    "license",
    "keywords",
    "ccrVersion",
    "source",
    "sourceType",
    "checksum",
];

/// 动态配置字段清单。
const DYNAMIC_FIELDS: &[&str] = &["schema", "template", "configMappings", "userValues"];

/// 校验预设名，避免路径穿越。
fn validate_preset_name(preset_name: &str) -> Result<(), SharedError> {
    if preset_name.trim().is_empty() {
        return Err(SharedError::InvalidPresetName("预设名不能为空".to_string()));
    }

    if preset_name.contains("..") || preset_name.contains('/') || preset_name.contains('\\') {
        return Err(SharedError::InvalidPresetName(format!(
            "预设名包含非法路径片段: {preset_name}"
        )));
    }

    if Path::new(preset_name).is_absolute() {
        return Err(SharedError::InvalidPresetName(format!(
            "预设名不能是绝对路径: {preset_name}"
        )));
    }

    Ok(())
}

/// 获取预设目录。
pub fn get_preset_dir(preset_name: &str) -> Result<PathBuf, SharedError> {
    validate_preset_name(preset_name)?;
    Ok(PRESETS_DIR.join(preset_name))
}

/// 获取临时目录。
pub fn get_temp_dir() -> PathBuf {
    HOME_DIR.join("temp")
}

/// 校验并规整解压目标路径。
fn validate_and_resolve_path(target_dir: &Path, entry_path: &str) -> Result<PathBuf, SharedError> {
    // 先做组件级校验，拒绝 `..`、绝对路径、盘符前缀等危险输入。
    for component in Path::new(entry_path).components() {
        use std::path::Component;
        match component {
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(SharedError::PathTraversal(entry_path.to_string()));
            }
            _ => {}
        }
    }

    Ok(target_dir.join(entry_path))
}

/// 将 zip 解压到目标目录。
pub async fn extract_preset(source_zip: &Path, target_dir: &Path) -> Result<(), SharedError> {
    if target_dir.exists() {
        return Err(SharedError::PresetAlreadyExists(
            target_dir.to_string_lossy().to_string(),
        ));
    }

    fs::create_dir_all(target_dir).await?;

    // ZIP API 目前是阻塞式，这里在同步上下文读取。
    let file = File::open(source_zip)?;
    let mut archive = ZipArchive::new(file)?;

    let mut root_dirs = HashSet::new();
    let mut has_manifest = false;
    for index in 0..archive.len() {
        let file = archive.by_index(index)?;
        let name = file.name().to_string();
        if name == "manifest.json" {
            has_manifest = true;
        }
        if let Some((root, _)) = name.split_once('/') {
            root_dirs.insert(root.to_string());
            if name == format!("{root}/manifest.json") {
                has_manifest = true;
            }
        }
    }

    let strip_root = root_dirs.len() == 1 && has_manifest;
    let single_root = if strip_root {
        root_dirs.iter().next().cloned()
    } else {
        None
    };

    for index in 0..archive.len() {
        let mut zip_file = archive.by_index(index)?;
        if zip_file.is_dir() {
            continue;
        }

        let mut entry_name = zip_file.name().to_string();
        if let Some(root) = &single_root {
            let prefix = format!("{root}/");
            if entry_name.starts_with(&prefix) {
                entry_name = entry_name.trim_start_matches(&prefix).to_string();
            }
            if entry_name.is_empty() {
                continue;
            }
        }

        let target_path = validate_and_resolve_path(target_dir, &entry_name)?;
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut data = Vec::new();
        zip_file.read_to_end(&mut data)?;
        let mut out = File::create(target_path)?;
        out.write_all(&data)?;
    }

    Ok(())
}

/// 读取目录中的 manifest。
pub async fn read_manifest_from_dir(preset_dir: &Path) -> Result<ManifestFile, SharedError> {
    let manifest_path = preset_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(SharedError::MissingFile(manifest_path));
    }

    let content = fs::read_to_string(&manifest_path).await?;
    json5::from_str::<ManifestFile>(&content).map_err(json5_err)
}

/// 将 manifest 转为 PresetFile。
pub fn manifest_to_preset_file(manifest: &ManifestFile) -> PresetFile {
    let mut metadata_map = serde_json::Map::new();
    let mut dynamic_map = serde_json::Map::new();
    let mut config_map = serde_json::Map::new();

    let raw = serde_json::to_value(manifest)
        .unwrap_or(Value::Object(Default::default()))
        .as_object()
        .cloned()
        .unwrap_or_default();

    for (key, value) in raw {
        if METADATA_FIELDS.contains(&key.as_str()) {
            metadata_map.insert(key, value);
        } else if DYNAMIC_FIELDS.contains(&key.as_str()) {
            dynamic_map.insert(key, value);
        } else {
            config_map.insert(key, value);
        }
    }

    let metadata = serde_json::from_value::<PresetMetadata>(Value::Object(metadata_map)).ok();
    let config = crate::preset::types::PresetConfigSection::from_value(Value::Object(config_map));

    PresetFile {
        metadata,
        config,
        secrets: None,
        schema: dynamic_map
            .get("schema")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        template: dynamic_map.get("template").cloned(),
        config_mappings: dynamic_map
            .get("configMappings")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
    }
}

/// 下载远程预设 zip 到临时目录。
pub async fn download_preset_to_temp(url: &str) -> Result<PathBuf, SharedError> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(SharedError::Validation(format!(
            "下载预设失败: {}",
            response.status()
        )));
    }

    let bytes = response.bytes().await?;
    let temp_dir = get_temp_dir();
    fs::create_dir_all(&temp_dir).await?;

    let file_path = temp_dir.join(format!("preset-{}.zip", chrono_like_timestamp()));
    fs::write(&file_path, bytes).await?;
    Ok(file_path)
}

/// 读取预设（目录路径或预设名）。
pub async fn load_preset(source: &str) -> Result<PresetFile, SharedError> {
    let source_path = Path::new(source);

    let manifest = if source.contains('/') || source.contains('\\') || source_path.is_absolute() {
        read_manifest_from_dir(source_path).await?
    } else {
        let dir = get_preset_dir(source)?;
        read_manifest_from_dir(&dir).await?
    };

    Ok(manifest_to_preset_file(&manifest))
}

/// 预设结构校验。
pub async fn validate_preset(preset: &PresetFile) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if preset.metadata.is_none() {
        warnings.push("缺少 metadata 段".to_string());
    }

    if let Some(providers) = &preset.config.providers {
        for provider in providers {
            if provider.name.trim().is_empty() {
                errors.push("Provider 缺少 name".to_string());
            }
            if provider.api_base_url.trim().is_empty() {
                errors.push(format!("Provider {} 缺少 api_base_url", provider.name));
            }
            if provider.models.is_empty() {
                warnings.push(format!("Provider {} 未声明 models", provider.name));
            }
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

/// 从 manifest 提取 metadata。
pub fn extract_metadata(manifest: &ManifestFile) -> Option<PresetMetadata> {
    match (&manifest.name, &manifest.version) {
        (Some(name), Some(version)) => Some(PresetMetadata {
            name: name.clone(),
            version: version.clone(),
            description: manifest.description.clone(),
            author: manifest.author.clone(),
            homepage: manifest.homepage.clone(),
            repository: manifest.repository.clone(),
            license: manifest.license.clone(),
            keywords: manifest.keywords.clone(),
            ccr_version: manifest.ccr_version.clone(),
            source: manifest.source.clone(),
            source_type: manifest.source_type.clone(),
            checksum: manifest.checksum.clone(),
        }),
        _ => None,
    }
}

/// 保存 manifest。
pub async fn save_manifest(preset_name: &str, manifest: &ManifestFile) -> Result<(), SharedError> {
    let preset_dir = get_preset_dir(preset_name)?;
    fs::create_dir_all(&preset_dir).await?;
    let path = preset_dir.join("manifest.json");
    let content = serde_json::to_string_pretty(manifest)?;
    fs::write(path, content).await?;
    Ok(())
}

/// 判断预设是否安装。
pub async fn is_preset_installed(preset_name: &str) -> Result<bool, SharedError> {
    let preset_dir = get_preset_dir(preset_name)?;
    Ok(preset_dir.exists())
}

/// 列出所有已安装预设。
pub async fn list_presets() -> Result<Vec<PresetInfo>, SharedError> {
    if !PRESETS_DIR.exists() {
        return Ok(vec![]);
    }

    let mut presets = Vec::new();
    let mut dir = fs::read_dir(PRESETS_DIR.as_path()).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("manifest.json");
        if !manifest_path.exists() {
            continue;
        }

        let content = match fs::read_to_string(&manifest_path).await {
            Ok(content) => content,
            Err(_) => continue,
        };

        let manifest = match json5::from_str::<ManifestFile>(&content) {
            Ok(manifest) => manifest,
            Err(_) => continue,
        };

        let config = load_config_from_manifest(&manifest, Some(&path));
        presets.push(PresetInfo {
            name: manifest
                .name
                .clone()
                .unwrap_or_else(|| entry.file_name().to_string_lossy().to_string()),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            author: manifest.author.clone(),
            config,
        });
    }

    Ok(presets)
}

/// 生成简易时间戳（避免引入 chrono）。
fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    millis.to_string()
}
