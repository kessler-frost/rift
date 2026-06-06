use std::time::Duration;

use thiserror::Error;

use crate::config::RiftAiConfig;
use crate::messages::{MessagesRequest, MessagesResponse};

#[derive(Debug, Error)]
pub enum AiError {
    #[error("request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("ai backend returned status {0}")]
    Status(u16),
}

/// POST a Messages request to `{endpoint}/v1/messages` and parse the response.
pub async fn send_messages(
    cfg: &RiftAiConfig,
    req: &MessagesRequest,
) -> Result<MessagesResponse, AiError> {
    let url = format!("{}/v1/messages", cfg.endpoint.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(cfg.timeout_ms))
        .build()?;
    let resp = client
        .post(url)
        .header("x-api-key", &cfg.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(req)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(AiError::Status(resp.status().as_u16()));
    }
    Ok(resp.json::<MessagesResponse>().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn cfg_for(endpoint: String) -> RiftAiConfig {
        RiftAiConfig::from_toml_str(&format!(
            "[ai]\nendpoint = \"{endpoint}\"\nmodel = \"m\"\n"
        ))
        .unwrap()
    }

    #[tokio::test]
    async fn posts_to_v1_messages_and_parses_text() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "content": [ { "type": "text", "text": "git status" } ]
            })))
            .mount(&server)
            .await;

        let cfg = cfg_for(server.uri());
        let req = MessagesRequest::single_user("m", "sys", "u", 64);
        let resp = send_messages(&cfg, &req).await.unwrap();
        assert_eq!(resp.text(), "git status");
    }

    #[tokio::test]
    async fn non_200_is_status_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let cfg = cfg_for(server.uri());
        let req = MessagesRequest::single_user("m", "sys", "u", 64);
        let err = send_messages(&cfg, &req).await.unwrap_err();
        assert!(matches!(err, AiError::Status(503)));
    }
}
