use anyhow::Result;
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use tracing::info;

use super::backend::LlmBackend;
use super::constructor_api::{CompletionParameters, CompletionResponse};

const END_POINT: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Clone, Debug)]
pub struct Llm {
    client: reqwest::Client,
    headers: HeaderMap,
}

impl Llm {
    pub fn try_new() -> Result<Self> {
        let token = std::env::var("OPENROUTER_TOKEN")?;
        let client = reqwest::Client::new();
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}").parse()?,
        );
        Ok(Self { client, headers })
    }

    async fn complete(&self, params: &CompletionParameters) -> Result<CompletionResponse> {
        let response = self
            .client
            .post(END_POINT)
            .headers(self.headers.clone())
            .json(params)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!(
                "request parameters: {:#?}\nresponse status: {}\nbody: {}",
                params,
                status,
                body
            );
        }

        let response_json = response.json().await?;
        info!(target: "llm", "response from openrouter:\t{:#?}", response_json);
        Ok(response_json)
    }
}

#[async_trait]
impl LlmBackend for Llm {
    async fn model_complete(&self, params: &CompletionParameters) -> Result<CompletionResponse> {
        self.complete(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[tokio::test]
    async fn live_completion() -> Result<()> {
        let llm = Llm::try_new()?;
        let params = CompletionParameters {
            model: "openai/gpt-oss-120b:free".to_string(), // free model on OpenRouter
            messages: vec![super::super::constructor_api::MessageOut::new_user(
                "Say hello in one word.".to_string(),
            )],
            ..Default::default()
        };
        let response = llm.complete(&params).await?;
        eprintln!("{:#?}", response);
        Ok(())
    }
}
