//! 读取已安装 preset 的快捷函数。

use tokio::fs;

use crate::error::{SharedError, json5_err};
use crate::preset::install::get_preset_dir;

/// 读取 preset 的 manifest 内容。
///
/// 返回值语义：
/// - `Ok(Some(v))`：读取成功。
/// - `Ok(None)`：文件不存在。
/// - `Err(e)`：其他错误。
pub async fn read_preset_file(name: &str) -> Result<Option<serde_json::Value>, SharedError> {
    let preset_dir = get_preset_dir(name)?;
    let manifest_path = preset_dir.join("manifest.json");

    if !manifest_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(manifest_path).await?;
    let value = json5::from_str::<serde_json::Value>(&content).map_err(json5_err)?;
    Ok(Some(value))
}
