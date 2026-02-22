use serde::{Deserialize, Serialize};
use crate::scanner::ScannerError;

const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: String,
    content: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Deserialize)]
struct ChatResponseMessage {
    content: String,
}

pub struct GroqClient {
    api_key: String,
    client: reqwest::Client,
}

impl GroqClient {
    pub fn new(api_key: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Send a text chat completion request
    pub async fn chat(
        &self,
        model: &str,
        system_prompt: &str,
        user_message: &str,
        temperature: Option<f64>,
        max_tokens: Option<u32>,
    ) -> Result<String, ScannerError> {
        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: serde_json::Value::String(system_prompt.to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content: serde_json::Value::String(user_message.to_string()),
                },
            ],
            temperature,
            max_tokens,
        };

        self.send_request(&request).await
    }

    /// Send a vision request with an image
    pub async fn vision(
        &self,
        model: &str,
        system_prompt: &str,
        user_text: &str,
        image_base64: &str,
    ) -> Result<String, ScannerError> {
        let content = serde_json::json!([
            {
                "type": "text",
                "text": user_text
            },
            {
                "type": "image_url",
                "image_url": {
                    "url": format!("data:image/png;base64,{}", image_base64)
                }
            }
        ]);

        let request = ChatRequest {
            model: model.to_string(),
            messages: vec![
                ChatMessage {
                    role: "system".to_string(),
                    content: serde_json::Value::String(system_prompt.to_string()),
                },
                ChatMessage {
                    role: "user".to_string(),
                    content,
                },
            ],
            temperature: Some(0.1),
            max_tokens: Some(4096),
        };

        self.send_request(&request).await
    }

    async fn send_request(&self, request: &ChatRequest) -> Result<String, ScannerError> {
        let response = self
            .client
            .post(GROQ_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| ScannerError::SystemError(format!("Groq API request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ScannerError::SystemError(format!(
                "Groq API error {}: {}",
                status, body
            )));
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .map_err(|e| ScannerError::SystemError(format!("Groq API parse error: {}", e)))?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| ScannerError::SystemError("Groq returned no choices".into()))
    }
}
