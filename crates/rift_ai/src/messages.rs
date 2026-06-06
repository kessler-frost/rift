use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// Anthropic Messages API request body.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct MessagesRequest {
    pub model: String,
    pub max_tokens: u32,
    pub system: String,
    pub messages: Vec<Message>,
}

impl MessagesRequest {
    pub fn single_user(model: &str, system: &str, user: &str, max_tokens: u32) -> Self {
        Self {
            model: model.to_string(),
            max_tokens,
            system: system.to_string(),
            messages: vec![Message { role: "user".into(), content: user.into() }],
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub text: String,
}

/// Anthropic Messages API response body (text blocks only — what omlx returns).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct MessagesResponse {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
}

impl MessagesResponse {
    /// Concatenate all text blocks, trimmed.
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter(|b| b.kind == "text")
            .map(|b| b.text.as_str())
            .collect::<String>()
            .trim()
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_serializes_to_anthropic_shape() {
        let req = MessagesRequest::single_user("m", "sys", "hi", 256);
        let v: serde_json::Value = serde_json::to_value(&req).unwrap();
        assert_eq!(v["model"], "m");
        assert_eq!(v["max_tokens"], 256);
        assert_eq!(v["system"], "sys");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["content"], "hi");
    }

    #[test]
    fn response_extracts_concatenated_text() {
        let json = r#"{"content":[{"type":"text","text":"  git status  "}]}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.text(), "git status");
    }

    #[test]
    fn response_ignores_non_text_blocks() {
        let json = r#"{"content":[{"type":"thinking","text":"x"},{"type":"text","text":"ls"}]}"#;
        let resp: MessagesResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.text(), "ls");
    }
}
