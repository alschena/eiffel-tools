use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Display;
#[derive(Serialize, Deserialize, Debug)]
pub(super) enum Role {
    #[serde(rename(deserialize = "user"))]
    User,
    #[serde(rename(deserialize = "model"))]
    Model,
}
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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
#[derive(Debug)]
pub struct Config {
    token: String,
    mode: Mode,
    model: Model,
}
impl Config {
    pub fn new(model: Model, mode: Mode, env_var_token: String) -> Config {
        let token = env::var(env_var_token).expect("Environment variable containing gemini token.");
        Config { token, mode, model }
    }
    pub fn end_point(&self) -> reqwest::Url {
        let model = self.model.clone();
        let mode = self.mode.clone();
        let end_point =
            format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:{mode}?key=")
                + self.token.as_str();
        end_point
            .as_str()
            .try_into()
            .expect("Initialize config Gemini")
    }
}
impl Default for Config {
    fn default() -> Self {
        let mode = Mode::Generate;
        let model = Model::Flash;
        let env_var_token = "GOOGLE_API_KEY".to_string();
        Self::new(model, mode, env_var_token)
    }
}
