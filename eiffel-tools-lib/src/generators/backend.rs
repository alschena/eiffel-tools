use anyhow::Result;
use async_trait::async_trait;

use super::constructor_api::{CompletionParameters, CompletionResponse};

#[async_trait]
pub trait LlmBackend: Send + Sync + std::fmt::Debug {
    async fn model_complete(&self, params: &CompletionParameters) -> Result<CompletionResponse>;
}
