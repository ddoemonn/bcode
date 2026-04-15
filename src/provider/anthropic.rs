use super::{Message, Provider, Role, StreamEvent};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

pub struct AnthropicProvider {
    key: String,
    model: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(key: String, model: String) -> Self {
        Self { key, model, client: Client::new() }
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Event {
    #[serde(rename = "message_start")]
    MessageStart { message: MessageStart },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: Delta },
    #[serde(rename = "message_delta")]
    MessageDelta { usage: MessageDeltaUsage },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessageStart {
    usage: StartUsage,
}

#[derive(Deserialize)]
struct StartUsage {
    input_tokens: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessageDeltaUsage {
    output_tokens: u32,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }
    fn model(&self) -> &str { &self.model }

    async fn stream(&self, messages: &[Message], tx: mpsc::Sender<StreamEvent>) -> anyhow::Result<()> {
        let system = messages.iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.clone());

        let msgs: Vec<serde_json::Value> = messages.iter()
            .filter(|m| m.role != Role::System)
            .map(|m| json!({
                "role": if m.role == Role::User { "user" } else { "assistant" },
                "content": m.content,
            }))
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
            "max_tokens": 8192,
            "stream": true,
        });

        if let Some(sys) = system {
            body["system"] = json!(sys);
        }

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .context("failed to connect to Anthropic API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamEvent::Error(format!("{status}: {text}"))).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut input_tokens = 0u32;

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => { let _ = tx.send(StreamEvent::Error(e.to_string())).await; break; }
            };

            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                for line in block.lines() {
                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    if data == "[DONE]" { continue }
                    let Ok(ev) = serde_json::from_str::<Event>(data) else { continue };

                    match ev {
                        Event::MessageStart { message } => {
                            input_tokens = message.usage.input_tokens;
                        }
                        Event::ContentBlockDelta { delta: Delta::Text { text } } => {
                            let _ = tx.send(StreamEvent::TextDelta(text)).await;
                        }
                        Event::MessageDelta { usage } => {
                            let _ = tx.send(StreamEvent::Done {
                                input_tokens,
                                output_tokens: usage.output_tokens,
                            }).await;
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
    }
}
