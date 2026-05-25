use anyhow::Result;
use anyhow::ensure;
use async_trait::async_trait;
use reqwest::header::HeaderMap;
use schemars::JsonSchema;
use schemars::schema_for;
use serde::Deserialize;
use serde::Serialize;
use tracing::info;

use super::backend::LlmBackend;

const END_POINT: &str = r#"https://training.constructor.app/api/platform-kmapi/v1"#;

#[derive(Serialize, Deserialize, Debug)]
struct ModelProvider {
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct LanguageModel {
    id: String,
    name: String,
    description: String,
    hosted_by: Option<ModelProvider>,
    code: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ListLanguageModels {
    results: Vec<LanguageModel>,
    total: i32,
}

#[derive(Serialize, Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
enum SharedTypes {
    #[default]
    Private,
    All,
    Tenant,
}

#[derive(Serialize, Debug)]
pub struct CreateKnowledgeModelParameters {
    name: String,
    description: String,
    shared_type: SharedTypes,
}

#[derive(Serialize, Deserialize, Debug)]
struct KnowledgeModelOwner {
    user_id: String,
    tenant_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct KnowledgeModel {
    id: String,
    name: String,
    description: Option<String>,
    owner: KnowledgeModelOwner,
    shared_type: SharedTypes,
    created_at: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct ListKnowledgeModels {
    results: Vec<KnowledgeModel>,
    total: i32,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct MessageOut {
    role: String,
    content: String,
    name: Option<String>,
}

impl MessageOut {
    pub fn new_system(content: String) -> MessageOut {
        MessageOut {
            role: "system".to_string(),
            content,
            name: Some("AutoProof's coding assistant.".to_string()),
        }
    }
    pub fn new_user(content: String) -> MessageOut {
        MessageOut {
            role: "user".to_string(),
            content,
            name: None,
        }
    }
}

#[derive(Serialize, Debug, Default, Clone)]
#[serde(rename_all = "snake_case")]
enum OpenAIResponseFormatOptions {
    #[default]
    JsonSchema,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct OpenAIJsonSchema {
    name: String,
    schema: schemars::schema::RootSchema,
    strict: bool,
}

impl OpenAIJsonSchema {
    #[allow(unused)]
    pub fn new<T: JsonSchema>() -> Self {
        let schema = schema_for!(T);
        let name = schema
            .schema
            .metadata
            .as_ref()
            .and_then(|meta| meta.title.clone())
            .unwrap_or_default();
        Self {
            name,
            schema,
            strict: true,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct OpenAIResponseFormat {
    r#type: OpenAIResponseFormatOptions,
    json_schema: OpenAIJsonSchema,
}

impl OpenAIResponseFormat {
    #[allow(unused)]
    pub fn json<T: JsonSchema>() -> Self {
        Self {
            r#type: OpenAIResponseFormatOptions::JsonSchema,
            json_schema: OpenAIJsonSchema::new::<T>(),
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct CompletionParameters {
    pub model: String,
    pub messages: Vec<MessageOut>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    // stop: String | Vec<String> | None
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    // property name*: Any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<OpenAIResponseFormat>,
}

impl Default for CompletionParameters {
    fn default() -> Self {
        Self {
            model: "claude-sonnet-4-0".to_string(),
            messages: Vec::new(),
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stream: false,
            tools: None,
            tool_choice: None,
            n: None,
            response_format: None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MessageReceived {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompletionChoice {
    pub index: usize,
    pub message: MessageReceived,
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompletionTokenUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    /// Any extra token-usage fields returned by the provider (e.g. prompt_tokens_details).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<CompletionChoice>,
    pub usage: CompletionTokenUsage,
    /// Any extra fields returned by the provider (pricing, routing info, etc.).
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl CompletionResponse {
    fn contents(&self) -> impl Iterator<Item = &str> {
        self.choices.iter().map(|c| c.message.content.as_str())
    }

    fn remove_quotes_around_markdown_code_block(content: &str) -> String {
        content
            .lines()
            .skip_while(|&line| {
                let line = line.trim_start();
                line.is_empty() || line.starts_with(r#"```"#)
            })
            .map_while(|line| (line.trim_end() != r#"```"#).then_some(line))
            .fold(String::new(), |mut acc, line| {
                acc.push_str(line);
                acc.push('\n');
                acc
            })
    }

    fn first_code_block_in_markdown(content: &str) -> String {
        Self::remove_quotes_around_markdown_code_block(
            content
                .lines()
                .skip_while(|&line| !line.trim_start().starts_with(r#"```"#))
                .fold(String::new(), |mut acc, line| {
                    acc.push_str(line);
                    acc.push('\n');
                    acc
                })
                .as_str(),
        )
    }

    pub fn markdown_to_code(&self) -> Vec<String> {
        self.contents()
            .map(Self::remove_quotes_around_markdown_code_block)
            .inspect(|content| info!("Extract from first markdown block in text: {content}"))
            .chain(
                self.contents()
                    .map(Self::first_code_block_in_markdown)
                    .inspect(|content| {
                        info!("Extract from first markdown block in text: {content}")
                    }),
            )
            .collect()
    }
}

pub struct LlmBuilder {
    client: reqwest::Client,
    headers: HeaderMap,
}
impl LlmBuilder {
    pub fn try_new() -> Result<Self> {
        let client = reqwest::Client::new();
        let token = std::env::var("CONSTRUCTOR_APP_API_TOKEN")?;
        let mut headers = HeaderMap::new();
        headers.insert("X-KM-AccessKey", format!("Bearer {token}").parse()?);
        Ok(Self { client, headers })
    }

    #[allow(unused)]
    async fn list_language_models(&self) -> Result<ListLanguageModels> {
        let response = self
            .client
            .get(format!("{END_POINT}/language_models"))
            .headers(self.headers.clone())
            .send()
            .await?;

        let list_language_models = response.json().await?;

        Ok(list_language_models)
    }

    async fn list_knowledge_models(&self) -> Result<ListKnowledgeModels> {
        let response = self
            .client
            .get(format!("{END_POINT}/knowledge-models"))
            .headers(self.headers.clone())
            .send()
            .await?;
        let list_knowledge_models_response = response.json().await?;

        Ok(list_knowledge_models_response)
    }

    async fn get_knowledge_model(&self, id: String) -> Result<KnowledgeModel> {
        let response = self
            .client
            .get(format!("{END_POINT}/knowledge-models/{id}"))
            .headers(self.headers.clone())
            .send()
            .await?;
        let knowledge_model = response.json().await?;
        Ok(knowledge_model)
    }

    async fn find_available_knowledge_model(&self) -> Result<Option<KnowledgeModel>> {
        match self.list_knowledge_models().await {
            Ok(list_knowledge_models) => match list_knowledge_models.results.first() {
                Some(first_result) => {
                    let id = first_result.id.clone();
                    let knowledge_model = self.get_knowledge_model(id).await?;
                    Ok(Some(knowledge_model))
                }
                None => Ok(None),
            },
            Err(e) => Err(e),
        }
    }

    async fn create_knowledge_model(
        &self,
        parameters: &CreateKnowledgeModelParameters,
    ) -> Result<KnowledgeModel> {
        let response = self
            .client
            .post(format!("{END_POINT}/knowledge-models"))
            .headers(self.headers.clone())
            .json(parameters)
            .send()
            .await?;
        let response_parsed = response.json().await?;
        Ok(response_parsed)
    }

    pub async fn build(self, parameters: &CreateKnowledgeModelParameters) -> Result<Llm> {
        let already_available_knowledge_model = self.find_available_knowledge_model().await?;

        let knowledge_model = match already_available_knowledge_model {
            Some(val) => val,
            None => self.create_knowledge_model(parameters).await?,
        };

        let knowledge_model_id = knowledge_model.id;

        Ok(Llm {
            client: self.client,
            headers: self.headers,
            knowledge_model_id,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Llm {
    client: reqwest::Client,
    headers: HeaderMap,
    knowledge_model_id: String,
}
impl Llm {
    pub async fn try_new() -> Result<Llm> {
        let builder = LlmBuilder::try_new()?;
        let parameters = CreateKnowledgeModelParameters {
            name: "Eiffel contract factory".to_string(),
            description: "Remote private inference for the `Eiffel contract factory` tool."
                .to_string(),
            shared_type: SharedTypes::All,
        };
        builder.build(&parameters).await
    }

    pub async fn model_complete(
        &self,
        parameters: &CompletionParameters,
    ) -> Result<CompletionResponse> {
        let knowledge_model_id = &self.knowledge_model_id;

        let request = self
            .client
            .post(format!(
                "{END_POINT}/knowledge-models/{knowledge_model_id}/chat/completions/direct_llm"
            ))
            .json(&parameters)
            .headers(self.headers.clone());

        let response = request.send().await?;

        ensure!(
            response.status().is_success(),
            "request parameters: {:#?}\n
            response status: {}",
            parameters,
            response.status()
        );

        let response_json = response.json().await?;

        info!(target: "llm", "response sent by llm:\t{:#?}", response_json);

        Ok(response_json)
    }
}

#[async_trait]
impl LlmBackend for Llm {
    async fn model_complete(&self, params: &CompletionParameters) -> Result<CompletionResponse> {
        self.model_complete(params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::code_entities::contract::RoutineSpecification;

    #[ignore]
    #[tokio::test]
    async fn private_inference_request() -> Result<()> {
        let llm_builder = LlmBuilder::try_new()?;

        // List knowledge models
        let list_language_models = llm_builder.list_language_models().await?;
        eprintln!("list knowledge models:\n{list_language_models:#?}");

        // Create knowledge model
        let parameters = CreateKnowledgeModelParameters {
            name: "Eiffel contract factory".to_string(),
            description: "Remote private inference for the `Eiffel contract factory` tool."
                .to_string(),
            shared_type: SharedTypes::All,
        };

        let llm = llm_builder.build(&parameters).await?;

        let knowledge_model_id = llm.knowledge_model_id.clone();
        eprintln!("create knowledge model id:\n{knowledge_model_id:#?}");

        let messages = vec![
            MessageOut{ role: "system".to_string(), content: "You are an experienced computer programmer in Eiffel. Respond only in eiffel code".to_string(), name: Some("DbC adviser".to_string()) },
            MessageOut{ role: "user".to_string(), content: "Write a function to compute the sum of a given integer array in Eiffel".to_string(), name: Some("DbC adviser".to_string()) },
        ];

        let data: CompletionParameters = CompletionParameters {
            messages,
            ..Default::default()
        };

        llm.model_complete(&data).await?;
        Ok(())
    }

    #[ignore]
    #[tokio::test]
    async fn structured_inference_request() -> Result<()> {
        let llm = Llm::try_new().await?;

        let knowledge_model_id = llm.knowledge_model_id.clone();
        eprintln!("create knowledge model id:\n{knowledge_model_id:#?}");

        let messages = vec![
            MessageOut{ role: "system".to_string(), content: "You are an experienced computer programmer in Eiffel, versed in design by contract. Respond only in eiffel code".to_string(), name: Some("DbC adviser".to_string()) },
            MessageOut{ role: "user".to_string(), content: "Write the specification for a function with this signature `sum(a_x, a_y: INTEGER): INTEGER`".to_string(), name: Some("DbC adviser".to_string()) },
        ];

        let response_schema = OpenAIResponseFormat::json::<RoutineSpecification>();

        eprintln!(
            "{}",
            serde_json::to_string_pretty(&response_schema).unwrap()
        );

        let data: CompletionParameters = CompletionParameters {
            messages,
            response_format: Some(response_schema),
            ..Default::default()
        };

        let output = llm.model_complete(&data).await?;

        for out in output.contents() {
            eprintln!("{out}");
        }
        Ok(())
    }

    impl MessageReceived {
        fn new(content: String) -> Self {
            Self {
                role: "assistant".to_string(),
                content,
                tool_calls: None,
            }
        }
    }

    impl CompletionChoice {
        fn new(content: String) -> Self {
            Self {
                index: 0,
                message: MessageReceived::new(content),
                finish_reason: Some("stop".to_string()),
            }
        }
    }

    impl CompletionResponse {
        fn new(content: String) -> Self {
            CompletionResponse {
                id: "".to_string(),
                object: "chat.completion".to_string(),
                created: 1,
                model: "dummy".to_string(),
                choices: vec![CompletionChoice::new(content)],
                usage: CompletionTokenUsage {
                    prompt_tokens: 0,
                    completion_tokens: 0,
                    total_tokens: 0,
                    extra: serde_json::Map::new(),
                },
                extra: serde_json::Map::new(),
            }
        }
    }

    #[test]
    fn extract_multiline_code() {
        let res = CompletionResponse::new("```eiffel\nsmaller (other: NEW_INTEGER): BOOLEAN\n\tdo\n\t\tResult := value < other.value\n\tensure\n\t\tResult = (value < other.value)\n\tend\n```".to_string());
        let multiline_code = res.markdown_to_code();
        let content = multiline_code.first().unwrap();
        assert_eq!(
            content,
            "smaller (other: NEW_INTEGER): BOOLEAN\n\tdo\n\t\tResult := value < other.value\n\tensure\n\t\tResult = (value < other.value)\n\tend\n"
        );
    }
}
