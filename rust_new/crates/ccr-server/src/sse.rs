//! SSE 解析与重写工具。
//!
//! 目标：
//! - 允许跨 chunk 拼接行。
//! - 对 `data: {json}` 负载执行 transformer 响应重写。
//! - 保持非 JSON 行与控制行（如 `[DONE]`）原样透传。

use ccr_core::ResponseRewriter;
use serde_json::Value;

use crate::error::ServerError;

/// SSE 行缓冲器。
#[derive(Debug, Default)]
pub struct SseLineBuffer {
    /// 未形成完整行的尾部残留。
    pending: String,
}

impl SseLineBuffer {
    /// 创建缓冲器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 推入新字节块，并返回所有已完成行（不含换行符）。
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(chunk);
        self.pending.push_str(&text);

        let mut lines = Vec::new();
        while let Some(newline_index) = self.pending.find('\n') {
            let mut line = self.pending[..newline_index].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            lines.push(line);
            self.pending.drain(..=newline_index);
        }

        lines
    }

    /// 消费尾部残留行。
    pub fn finish(&mut self) -> Option<String> {
        if self.pending.is_empty() {
            return None;
        }

        let mut last_line = std::mem::take(&mut self.pending);
        if last_line.ends_with('\r') {
            last_line.pop();
        }
        Some(last_line)
    }
}

/// 重写单行 SSE 文本。
///
/// 行处理规则：
/// - 非 `data:` 行：原样返回。
/// - `data: [DONE]`：原样返回。
/// - `data: <非 JSON>`：原样返回。
/// - `data: <JSON>`：执行 transformer 响应重写后返回。
pub async fn rewrite_sse_line(
    line: &str,
    response_rewriter: &ResponseRewriter,
) -> Result<String, ServerError> {
    let Some(payload) = line.strip_prefix("data:") else {
        return Ok(line.to_string());
    };

    let payload = payload.trim_start();
    if payload == "[DONE]" || payload.is_empty() {
        return Ok(line.to_string());
    }

    let value: Value = match serde_json::from_str(payload) {
        Ok(value) => value,
        Err(_) => return Ok(line.to_string()),
    };

    let rewritten = response_rewriter.rewrite(&value).await?;
    let rewritten_text = serde_json::to_string(&rewritten)
        .map_err(|err| ServerError::Internal(format!("序列化 SSE 重写结果失败: {err}")))?;
    Ok(format!("data: {rewritten_text}"))
}

#[cfg(test)]
mod tests {
    use ccr_config::{ConfigOptions, ConfigService};
    use ccr_core::CoreService;
    use serde_json::json;

    use super::{SseLineBuffer, rewrite_sse_line};

    /// 构造带 pipeline 的 response_rewriter。
    async fn build_rewriter() -> ccr_core::ResponseRewriter {
        let config_service = ConfigService::new(ConfigOptions {
            use_json_file: false,
            use_env_file: false,
            use_environment_variables: false,
            initial_config: Some(json!({
                "providers": [
                    {
                        "name": "openai",
                        "api_base_url": "https://api.openai.com/v1/chat/completions",
                        "api_key": "sk-openai",
                        "models": ["gpt-5"],
                        "transformer": {
                            "use": [
                                ["sampling", {"temperature": 0.1}]
                            ]
                        }
                    }
                ],
                "Router": {
                    "default": "openai,gpt-5"
                }
            })),
            ..ConfigOptions::default()
        })
        .expect("create config service");

        let mut core_service = CoreService::new(config_service);
        let mut request_body = json!({
            "model": "openai,gpt-5",
            "messages": [{"role": "user", "content": "hello"}],
            "stream": false
        });

        let prepared = core_service
            .prepare_request(&mut request_body, None)
            .await
            .expect("prepare request");

        core_service
            .build_response_rewriter(&prepared)
            .expect("build rewriter")
    }

    #[test]
    fn test_sse_line_buffer_cross_chunk() {
        let mut buffer = SseLineBuffer::new();
        let lines_first = buffer.push_chunk(b"data: {\"a\":1}\n");
        assert_eq!(lines_first, vec!["data: {\"a\":1}".to_string()]);

        let lines_second = buffer.push_chunk(b"data: {\"b\":2}");
        assert!(lines_second.is_empty());

        let lines_third = buffer.push_chunk(b"\n\n");
        assert_eq!(
            lines_third,
            vec!["data: {\"b\":2}".to_string(), "".to_string()]
        );
    }

    #[tokio::test]
    async fn test_rewrite_sse_line_json_payload() {
        let rewriter = build_rewriter().await;

        let rewritten = rewrite_sse_line("data: {\"id\":\"x\",\"choices\":[]}", &rewriter)
            .await
            .expect("rewrite line");

        assert!(rewritten.starts_with("data: {"));
    }

    #[tokio::test]
    async fn test_rewrite_sse_line_done_passthrough() {
        let rewriter = build_rewriter().await;
        let rewritten = rewrite_sse_line("data: [DONE]", &rewriter)
            .await
            .expect("rewrite done line");
        assert_eq!(rewritten, "data: [DONE]");
    }
}
