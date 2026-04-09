//! 插件协议占位层。

use async_trait::async_trait;

/// 插件生命周期接口。
#[async_trait]
pub trait CcrPlugin: Send + Sync {
    /// 插件唯一名称。
    fn name(&self) -> &str;

    /// 插件初始化。
    async fn initialize(&mut self) -> Result<(), String> {
        Ok(())
    }

    /// 请求前回调。
    async fn before_request(
        &self,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(payload.clone())
    }

    /// 响应后回调。
    async fn after_response(
        &self,
        payload: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        Ok(payload.clone())
    }
}
