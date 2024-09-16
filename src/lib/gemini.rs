use reqwest;
use std::fmt::format;
use std::fmt::Display;
/// Required. The content of the current conversation with the model.
/// For single-turn queries, this is a single instance.
/// For multi-turn queries like chat, this is a repeated field that contains the conversation history and the latest request.
struct Contents {
    parts: Vec<Parts>,
}
struct Parts(Vec<String>);

/// Optional. A list of Tools the Model may use to generate the next response.
/// A Tool is a piece of code that enables the system to interact with external systems to perform an action, or set of actions, outside of knowledge and scope of the Model. Supported Tools are Function and codeExecution. Refer to the Function calling and the Code execution guides to learn more.
struct Tools;
/// Optional. Tool configuration for any Tool specified in the request. Refer to the Function calling guide for a usage example.
struct ToolConfig;
///Optional. A list of unique SafetySetting instances for blocking unsafe content.
///This will be enforced on the GenerateContentRequest.contents and GenerateContentResponse.candidates. There should not be more than one setting for each SafetyCategory type. The API will block any contents and responses that fail to meet the thresholds set by these settings. This list overrides the default settings for each SafetyCategory specified in the safetySettings. If there is no SafetySetting for a given SafetyCategory provided in the list, the API will use the default safety setting for that category. Harm categories HARM_CATEGORY_HATE_SPEECH, HARM_CATEGORY_SEXUALLY_EXPLICIT, HARM_CATEGORY_DANGEROUS_CONTENT, HARM_CATEGORY_HARASSMENT are supported. Refer to the guide for detailed information on available safety settings. Also refer to the Safety guidance to learn how to incorporate safety considerations in your AI applications.
struct SafetySetting;
/// Optional. Developer set system instruction(s). Currently, text only.
struct SystemInstruction;
/// Optional. Configuration options for model generation and outputs.
struct GenerationConfig;
/// Optional. The name of the content cached to use as context to serve the prediction. Format: cachedContents/{cachedContent}
struct CachedContent;
struct Request {
    contents: Contents,
    tools: Option<Tools>,
    toolConfig: Option<ToolConfig>,
    safetySettings: Option<Vec<SafetySetting>>,
    systemInstruction: Option<SystemInstruction>,
    generationConfig: Option<GenerationConfig>,
    cachedContent: Option<CachedContent>,
}
/// Candidate responses from the model.
struct Candidates(Vec<Candidate>);
struct Candidate(String);
/// Returns the prompt's feedback related to the content filters.
struct PromptFeedback;
/// Output only. Metadata on the generation requests' token usage.
struct UsageMetadata;
struct Response {}
enum Mode {
    Generate,
    Stream,
}
impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Mode::Generate => write!(f, "generateContent"),
            Mode::Stream => write!(f, "streamGenerateContent"),
        }
    }
}
enum Model {
    Flash,
    Pro,
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Model::Flash => write!(f, "gemini-1.5-flash"),
            Model::Pro => write!(f, "gemini-1.5-pro"),
        }
    }
}
struct Config {
    end_point: reqwest::Url,
    mode: Mode,
    model: Model,
}
impl Config {
    fn initialize(model: Model, mode: Mode) -> Config {
        let string_end_point = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:{mode}?key=$GOOGLE_API_KEY");
        match string_end_point.as_str().try_into() {
            Ok(end_point) => Config {
                end_point,
                mode,
                model,
            },
            Err(e) => {
                panic!()
            }
        }
    }
}
/// Interface of Gemini
struct Gemini {
    response: Option<Response>,
    request: Option<Request>,
    config: Config,
}
impl Gemini {
    /// Set request.
    fn request(&mut self, req: Request) {
        self.request = Some(req);
    }
    /// Get response.
    fn response(&self) -> &Option<Response> {
        &self.response
    }
}
impl Default for Gemini {
    fn default() -> Self {
        let response = None;
        let request = None;
        let model = Model::Flash;
        let mode = Mode::Generate;
        let config = Config::initialize(model, mode);
        Self {
            response,
            request,
            config,
        }
    }
}
