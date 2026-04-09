//! HuggingFace tokenizer 占位实现。
//!
//! 该实现复用 SimpleTokenizer 的计数策略，
//! 但保留了模型维度，后续可平滑替换为真实 HF tokenizer。

use async_trait::async_trait;
use ccr_protocol::{TokenizeRequest, Tokenizer};

use super::simple::SimpleTokenizer;

/// HuggingFace tokenizer（当前为可执行占位版）。
pub struct HuggingFaceTokenizer {
    /// 对齐配置中的 model 字段。
    model: String,
    /// 内部复用的简单 tokenizer。
    inner: SimpleTokenizer,
    /// 初始化状态。
    initialized: bool,
}

impl HuggingFaceTokenizer {
    /// 创建实例。
    pub fn new(model: impl Into<String>) -> Self {
        let model = model.into();
        Self {
            model: model.clone(),
            inner: SimpleTokenizer::new(format!("hf:{model}")),
            initialized: false,
        }
    }
}

#[async_trait]
impl Tokenizer for HuggingFaceTokenizer {
    fn tokenizer_type(&self) -> &str {
        "huggingface"
    }

    fn name(&self) -> &str {
        &self.model
    }

    async fn initialize(&mut self) -> Result<(), String> {
        self.initialized = true;
        self.inner.initialize().await
    }

    async fn count_tokens(&self, request: &TokenizeRequest) -> Result<u64, String> {
        self.inner.count_tokens(request).await
    }

    fn is_initialized(&self) -> bool {
        self.initialized
    }

    fn dispose(&mut self) {
        self.initialized = false;
        self.inner.dispose();
    }
}
