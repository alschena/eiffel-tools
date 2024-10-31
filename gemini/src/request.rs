use super::model;
use super::response;
use anyhow::{anyhow, Context, Result};
use config::GenerationConfig;
use reqwest;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
pub mod config;
/// The content of the current conversation with the model.
/// For single-turn queries, this is a single instance.
/// For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
#[derive(Serialize, Debug)]
pub struct Contents {
    parts: Vec<Part>,
}
impl FromStr for Contents {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = vec![s.parse().expect("Parse string into LSP-type `Part`")];
        Ok(Self { parts })
    }
}
impl From<String> for Contents {
    fn from(value: String) -> Self {
        let parts = vec![value.into()];
        Self { parts }
    }
}
/// For single-turn queries, it is the user's latest written request.
/// For multi-turn queries like chat, each part collects a snapshot of the conversation history or the latest request.
#[derive(Deserialize, Serialize, Debug)]
pub(super) struct Part {
    text: String,
}
impl Part {
    pub fn text(&self) -> &str {
        &self.text
    }
}
impl FromStr for Part {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Part {
            text: s.to_string(),
        })
    }
}
impl From<String> for Part {
    fn from(value: String) -> Self {
        let text = value;
        Self { text }
    }
}

/// A list of Tools the Model may use to generate the next response.
/// A Tool is a piece of code that enables the system to interact with external systems to perform an action, or set of actions, outside of knowledge and scope of the Model.
/// Supported Tools are Function and codeExecution.
/// Refer to the Function calling and the Code execution guides to learn more.
#[derive(Serialize, Debug)]
pub struct Tools;
/// Tool configuration for any Tool specified in the request.
/// Refer to the Function calling guide for a usage example.
#[derive(Serialize, Debug)]
pub struct ToolConfig;
/// A list of unique SafetySetting instances for blocking unsafe content.
/// This will be enforced on the GenerateContentRequest.
/// contents and GenerateContentResponse.
/// candidates.
/// There should not be more than one setting for each SafetyCategory type.
/// The API will block any contents and responses that fail to meet the thresholds set by these settings.
/// This list overrides the default settings for each SafetyCategory specified in the safetySettings.
/// If there is no SafetySetting for a given SafetyCategory provided in the list, the API will use the default safety setting for that category.
/// Harm categories HARM_CATEGORY_HATE_SPEECH, HARM_CATEGORY_SEXUALLY_EXPLICIT, HARM_CATEGORY_DANGEROUS_CONTENT, HARM_CATEGORY_HARASSMENT are supported.
/// Refer to the guide for detailed information on available safety settings.
/// Also refer to the Safety guidance to learn how to incorporate safety considerations in your AI applications.
#[derive(Serialize, Debug)]
pub struct SafetySetting;
/// Developer set system instruction(s).
/// Currently, text only.
#[derive(Serialize, Debug)]
pub struct SystemInstruction {
    content: Content,
}
impl FromStr for SystemInstruction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let content = s.parse().expect("Parse string into LSP type `Content`");
        Ok(Self { content })
    }
}
/// The name of the content cached to use as context to serve the prediction. Format: cachedContents/{cachedContent}
#[derive(Serialize, Debug)]
pub struct CachedContent;
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Request {
    contents: Contents,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Tools>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_config: Option<ToolConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_settings: Option<Vec<SafetySetting>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<SystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<config::GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cached_content: Option<CachedContent>,
}
impl Request {
    pub fn set_config(&mut self, config: GenerationConfig) {
        self.generation_config = Some(config)
    }
}
impl Request {
    pub fn new_async_client() -> reqwest::Client {
        reqwest::Client::new()
    }
    pub async fn process_with_async_client(
        self,
        config: model::Config,
        client: reqwest::Client,
    ) -> Result<response::Response> {
        let json_req = client.post(config.end_point()).json(&self);
        let res = json_req
            .send()
            .await
            .map_err(|e| anyhow!("fails to send response to gemini with error: {}", e))?;
        res.json::<response::Response>()
            .await
            .map_err(|e| anyhow!("fails to retrieve response from gemini with error: {}", e))
    }
    pub fn new_blocking_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::new()
    }
    pub fn process_with_blocking_client(
        &self,
        config: &model::Config,
        client: &reqwest::blocking::Client,
    ) -> Result<response::Response> {
        let json_req = client.post(config.end_point().clone()).json(&self);
        eprintln!("{:?}", json_req);
        let res = json_req
            .send()
            .map_err(|e| anyhow!("fails to retrieve response from gemini with error: {}", e))?;
        let response = res
            .json::<response::Response>()
            .expect("Decode response from Gemini");
        Ok(response)
    }
}
impl FromStr for Request {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let contents = s.parse().map_err(|e| {
            anyhow!(
                "fails to parse string to LSP-type contents with error: {:#?}",
                e
            )
        })?;
        Ok(Self {
            contents,
            tools: None,
            tool_config: None,
            safety_settings: None,
            system_instruction: None,
            generation_config: None,
            cached_content: None,
        })
    }
}
impl From<String> for Request {
    fn from(value: String) -> Self {
        let contents = value.into();
        Self {
            contents,
            tools: None,
            tool_config: None,
            safety_settings: None,
            system_instruction: None,
            generation_config: None,
            cached_content: None,
        }
    }
}
#[derive(Serialize, Deserialize, Debug)]
struct Content {
    parts: Vec<Part>,
    role: Option<Role>,
}
impl Default for Content {
    fn default() -> Self {
        let parts = Vec::new();
        let role = Some(Role::Model);
        Self { parts, role }
    }
}
impl FromStr for Content {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = vec![s.parse().expect("Parse string into LSP type `Part`")];
        Ok(Self {
            parts,
            ..Default::default()
        })
    }
}
#[derive(Serialize, Deserialize, Debug)]
enum Role {
    #[serde(rename(deserialize = "user"))]
    User,
    #[serde(rename(deserialize = "model"))]
    Model,
}

#[cfg(test)]
mod test {
    use super::*;
    use crate as gemini;
    use crate::{Described, ResponseSchema};
    use anyhow::Result;
    use config::schema::ToResponseSchema;
    use gemini_macro_derive::ToResponseSchema;
    #[test]
    fn serialize_simple_request() {
        let str_req = "Write a story about turles from the prospective of a frog.";
        let req = Request::from(str_req.to_string());
        assert_eq!(
            req.contents.parts.first().map(|p| p.text.clone()),
            Some(str_req.to_string())
        );
        assert!(req.tools.is_none());
        assert!(req.tool_config.is_none());
        assert!(req.safety_settings.is_none());
        assert!(req.system_instruction.is_none());
        assert!(req.generation_config.is_none());
        assert!(req.cached_content.is_none());
    }
    #[test]
    fn set_request_config() -> Result<()> {
        let mut req = Request::from_str("Tell me about the stars.")?;
        req.set_config(GenerationConfig::from(String::to_response_schema()));
        assert!(req.generation_config.is_some());
        assert_eq!(
            req.generation_config
                .map(|ref c| c.response_schema().clone()),
            Some(Some(String::to_response_schema()))
        );
        Ok(())
    }
    #[test]
    fn process_with_blocking_client() -> Result<()> {
        let model_config = model::Config::default();
        let client = Request::new_blocking_client();
        let req = Request::from_str("Tell me about the night.")?;
        let _ = req.process_with_blocking_client(&model_config, &client)?;
        Ok(())
    }
    #[tokio::test]
    async fn process_with_async_client() -> Result<()> {
        let model_config = model::Config::default();
        let client = Request::new_async_client();
        let req = Request::from_str("Tell me about the night.")?;
        let _ = req.process_with_async_client(model_config, client).await?;
        Ok(())
    }
    #[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Clone, Hash)]
    struct NightStory {
        today: String,
        tomorrow: String,
    }
    impl Described for NightStory {
        fn description() -> String {
            String::from("A story of the night")
        }
    }
    #[test]
    fn parse_res_with_custom_schema() -> Result<()> {
        let model_config = model::Config::default();
        let client = Request::new_blocking_client();
        let mut req = Request::from_str("Tell me about the night for today and tomorrow.")?;
        req.set_config(GenerationConfig::from(NightStory::to_response_schema()));
        let res = req.process_with_blocking_client(&model_config, &client)?;
        for r in res.parsable_content() {
            let n = serde_json::from_str::<NightStory>(r)?;
            eprintln!("{:#?}", n);
            assert!(!n.today.is_empty());
            assert!(!n.tomorrow.is_empty());
        }
        Ok(())
    }
}
