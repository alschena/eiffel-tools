use super::model;
use super::response;
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
    pub fn set_contents(&mut self, contents: Contents) {
        self.contents = contents;
    }
    pub fn set_tools(&mut self, tools: Option<Tools>) {
        self.tools = tools;
    }
    pub fn set_tool_config(&mut self, tool_config: Option<ToolConfig>) {
        self.tool_config = tool_config;
    }
    pub fn set_safety_settings(&mut self, safety_settings: Option<Vec<SafetySetting>>) {
        self.safety_settings = safety_settings;
    }
    pub fn set_system_instruction(&mut self, system_instruction: Option<SystemInstruction>) {
        self.system_instruction = system_instruction;
    }
    pub fn set_generation_config(&mut self, generation_config: Option<config::GenerationConfig>) {
        self.generation_config = generation_config;
    }
    pub fn set_cached_content(&mut self, cached_content: Option<CachedContent>) {
        self.cached_content = cached_content;
    }
}
impl Request {
    pub async fn process(&self, config: &model::Config) -> response::Response {
        let web_client = reqwest::Client::new();
        let json_req = web_client.post(config.end_point().clone()).json(&self);
        eprintln!("{:?}", json_req);
        match json_req.send().await {
            Ok(res) => {
                let response = res
                    .json::<response::Response>()
                    .await
                    .expect("Decode response from Gemini");
                response
            }
            Err(_) => {
                panic!("Response of Gemini")
            }
        }
    }
}
impl FromStr for Request {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let contents = s.parse().expect("Parse string to LSP-type contents");
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
    #[test]
    fn serialize_simple_request() {
        let req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        eprintln!("{:?}", serde_json::to_string(&req));
    }
    #[test]
    fn serialize_request_json_config() {
        let mut req =
            Request::from("Write a story about turles from the prospective of a frog.".to_string());
        let mut generation_config = config::GenerationConfig::default();
        generation_config.set_response_mime_type(Some(config::ResponseMimeType::Json));
        assert!(generation_config.response_mime_type() == &Some(config::ResponseMimeType::Json));
        generation_config.set_response_schema(Some(config::ResponseSchema::contracts()));
        req.set_generation_config(Some(generation_config));
        eprintln!("{:?}", serde_json::to_string(&req));
    }
}
