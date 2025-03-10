use super::constructor_api::CompletionResponse;

impl CompletionResponse {
    pub fn extract_multiline_code(&self) -> Vec<String> {
        self.contents()
            .map(|content| {
                content
                    .lines()
                    .skip_while(|&line| {
                        let line = line.trim_start();
                        let val = line.is_empty() || line.starts_with(r#"```"#);
                        val
                    })
                    .map_while(|line| (!(line.trim_end() == r#"```"#)).then_some(line))
                    .fold(String::new(), |mut acc, line| {
                        acc.push_str(line);
                        acc.push('\n');
                        acc
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::lib::generators::constructor_api::{
        CompletionChoice, CompletionTokenUsage, MessageReceived,
    };

    use super::CompletionResponse;

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
                },
            }
        }
    }

    #[test]
    fn extract_multiline_code() {
        let res = CompletionResponse::new("```eiffel\nsmaller (other: NEW_INTEGER): BOOLEAN\n\tdo\n\t\tResult := value < other.value\n\tensure\n\t\tResult = (value < other.value)\n\tend\n```".to_string());
        let multiline_code = res.extract_multiline_code();
        let multiline_code = multiline_code.first().unwrap();
        assert_eq!(multiline_code, "smaller (other: NEW_INTEGER): BOOLEAN\n\tdo\n\t\tResult := value < other.value\n\tensure\n\t\tResult = (value < other.value)\n\tend\n");
    }
}
