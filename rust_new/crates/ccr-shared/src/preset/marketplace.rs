//! Preset 市场访问。

use crate::error::SharedError;
use crate::preset::types::PresetIndexEntry;

/// 官方市场地址。
const MARKET_URL: &str = "https://pub-0dc3e1677e894f07bbea11b17a29e032.r2.dev/presets.json";

/// 拉取市场数据。
async fn fetch_market_data() -> Result<Vec<PresetIndexEntry>, SharedError> {
    let response = reqwest::get(MARKET_URL).await?;
    if !response.status().is_success() {
        return Err(SharedError::Validation(format!(
            "拉取 preset 市场失败: {}",
            response.status()
        )));
    }

    let entries = response.json::<Vec<PresetIndexEntry>>().await?;
    Ok(entries)
}

/// 获取全部市场预设。
pub async fn get_market_presets() -> Result<Vec<PresetIndexEntry>, SharedError> {
    fetch_market_data().await
}

/// 通过 id/name 查询市场预设。
pub async fn find_market_preset_by_name(
    preset_name: &str,
) -> Result<Option<PresetIndexEntry>, SharedError> {
    let presets = get_market_presets().await?;

    // 先按 id 精确匹配。
    if let Some(found) = presets.iter().find(|item| item.id == preset_name) {
        return Ok(Some(found.clone()));
    }

    // 再按 name 精确匹配。
    if let Some(found) = presets.iter().find(|item| item.name == preset_name) {
        return Ok(Some(found.clone()));
    }

    // 最后按 name 忽略大小写匹配。
    let target = preset_name.to_lowercase();
    Ok(presets
        .into_iter()
        .find(|item| item.name.to_lowercase() == target))
}
