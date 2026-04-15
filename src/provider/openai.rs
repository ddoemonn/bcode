use super::{Message, Provider, Role, StreamEvent};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

pub struct OpenAIProvider {
    key: String,
    model: String,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(key: String, model: String) -> Self {
        Self { key, model, client: Client::new() }
    }
}

#[derive(Deserialize)]
struct Chunk {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    delta: DeltaContent,
}

#[derive(Deserialize)]
struct DeltaContent {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str { "openai" }
    fn model(&self) -> &str { &self.model }

    async fn stream(&self, messages: &[Message], tx: mpsc::Sender<StreamEvent>) -> anyhow::Result<()> {
        let msgs: Vec<serde_json::Value> = messages.iter()
            .map(|m| json!({
                "role": match m.role { Role::User => "user", Role::Assistant => "assistant", Role::System => "system" },
                "content": m.content,
            }))
            .collect();

        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.key)
            .json(&json!({
                "model": self.model,
                "messages": msgs,
                "stream": true,
                "stream_options": { "include_usage": true },
            }))
            .send()
            .await
            .context("failed to connect to OpenAI API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamEvent::Error(format!("{status}: {text}"))).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut input_tokens = 0u32;
        let mut output_tokens = 0u32;

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
                    if data == "[DONE]" {
                        let _ = tx.send(StreamEvent::Done { input_tokens, output_tokens }).await;
                        continue;
                    }
                    let Ok(chunk) = serde_json::from_str::<Chunk>(data) else { continue };

                    if let Some(u) = chunk.usage {
                        input_tokens = u.prompt_tokens;
                        output_tokens = u.completion_tokens;
                    }
                    for choice in chunk.choices {
                        if let Some(text) = choice.delta.content {
                            if !text.is_empty() {
                                let _ = tx.send(StreamEvent::TextDelta(text)).await;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
