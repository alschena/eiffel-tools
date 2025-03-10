use reqwest::header::HeaderMap;
use schemars::schema_for;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;

const END_POINT: &'static str = r#"https://training.constructor.app/api/platform-kmapi/v1"#;

#[derive(Serialize, Deserialize, Debug)]
struct ModelProvider {
    name: String,
}

#[allow(unused)]
#[derive(Serialize, Debug, Default, Clone)]
pub enum EnumLanguageModel {
    #[serde(rename = "gemini-2.0-flash-001")]
    GeminiFlash,
    #[serde(rename = "gemini-1.5-pro")]
    GeminiPro,
    #[serde(rename = "learnlm-1.5-pro-experimental")]
    LearnlmProExperimental,
    #[serde(rename = "claude-3-opus-20240229")]
    ClaudeOpus,
    #[serde(rename = "claude-3-5-haiku-20241022")]
    ClaudeHaiku,
    #[serde(rename = "claude-3-7-sonnet-20250219")]
    ClaudeSonnet,
    #[serde(rename = "deepseek/deepseek-chat")]
    DeepSeekChat,
    #[serde(rename = "deepseek/deepseek-r1")]
    DeepSeekR1,
    #[default]
    #[serde(rename = "gpt-4o-mini")]
    Gpt4OMini,
    #[serde(rename = "gpt-4o-2024-08-06")]
    Gpt40,
    #[serde(rename = "o1-2024-12-17")]
    O1,
    #[serde(rename = "o3-mini")]
    O3Mini,
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
            name: None,
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
    pub fn new<T: JsonSchema>() -> Self {
        let schema = schema_for!(T);
        let name = schema
            .schema
            .metadata
            .as_ref()
            .map(|meta| meta.title.as_ref().map(|title| title.clone()))
            .flatten()
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
    pub fn json<T: JsonSchema>() -> Self {
        Self {
            r#type: OpenAIResponseFormatOptions::JsonSchema,
            json_schema: OpenAIJsonSchema::new::<T>(),
        }
    }
}

#[derive(Serialize, Debug, Default, Clone)]
pub struct CompletionParameters {
    pub model: EnumLanguageModel,
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

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct MessageReceived {
    pub(super) role: String,
    pub(super) content: String,
    // Currently always Null, but might change later.
    pub(super) tool_calls: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct CompletionChoice {
    pub(super) index: usize,
    pub(super) message: MessageReceived,
    pub(super) finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(super) struct CompletionTokenUsage {
    pub(super) prompt_tokens: i32,
    pub(super) completion_tokens: i32,
    pub(super) total_tokens: i32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CompletionResponse {
    pub(super) id: String,
    /// Response schema. Currently only "chat.completion" is allowed.
    pub(super) object: String,
    pub(super) created: i32,
    pub(super) model: String,
    pub(super) choices: Vec<CompletionChoice>,
    pub(super) usage: CompletionTokenUsage,
}

impl CompletionResponse {
    pub fn contents<'s>(&'s self) -> impl Iterator<Item = &'s str> + use<'s> {
        self.choices.iter().map(|c| c.message.content.as_str())
    }
}

pub struct LLMBuilder {
    client: reqwest::Client,
    headers: HeaderMap,
}
impl LLMBuilder {
    pub fn try_new() -> anyhow::Result<Self> {
        let client = reqwest::Client::new();
        let token = std::env::var("CONSTRUCTOR_APP_API_TOKEN")?;
        let mut headers = HeaderMap::new();
        headers.insert("X-KM-AccessKey", format!("Bearer {token}").parse()?);
        Ok(Self { client, headers })
    }
    async fn list_language_models(&self) -> anyhow::Result<ListLanguageModels> {
        let response = self
            .client
            .get(format!("{END_POINT}/language_models"))
            .headers(self.headers.clone())
            .send()
            .await?;

        let list_language_models = response.json().await?;

        Ok(list_language_models)
    }

    async fn create_knowledge_model(
        &self,
        parameters: &CreateKnowledgeModelParameters,
    ) -> anyhow::Result<KnowledgeModel> {
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
    pub async fn build(self, parameters: &CreateKnowledgeModelParameters) -> anyhow::Result<LLM> {
        let knowledge_model_id = self.create_knowledge_model(parameters).await?.id;

        Ok(LLM {
            client: self.client,
            headers: self.headers,
            knowledge_model_id,
        })
    }
}

#[derive(Clone, Debug)]
pub struct LLM {
    client: reqwest::Client,
    headers: HeaderMap,
    knowledge_model_id: String,
}
impl LLM {
    pub async fn try_new() -> anyhow::Result<LLM> {
        let builder = LLMBuilder::try_new()?;
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
    ) -> anyhow::Result<CompletionResponse> {
        let knowledge_model_id = &self.knowledge_model_id;

        let response = self
            .client
            .post(format!(
                "{END_POINT}/knowledge-models/{knowledge_model_id}/chat/completions"
            ))
            .json(&parameters)
            .headers(self.headers.clone())
            .send()
            .await?;

        debug_assert!(response.status().is_success(), "{}", response.text().await?);

        let response_json = response.json().await?;
        Ok(response_json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::code_entities::contract::RoutineSpecification;

    #[ignore]
    #[tokio::test]
    async fn private_inference_request() -> anyhow::Result<()> {
        let llm_builder = LLMBuilder::try_new()?;

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
    async fn structured_inference_request() -> anyhow::Result<()> {
        let llm = LLM::try_new().await?;

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
}
