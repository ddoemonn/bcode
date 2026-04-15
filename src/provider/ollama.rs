use super::{openai_tool_schemas, Provider, StreamEvent, ToolCallData};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::mpsc;

pub struct OllamaProvider {
    model: String,
    base_url: String,
    client: Client,
}

impl OllamaProvider {
    pub fn new(model: String) -> Self {
        let base_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        Self { model, base_url, client: Client::new() }
    }
}

#[derive(Deserialize)]
struct OllamaMsg {
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Deserialize)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Deserialize)]
struct OllamaFunction {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Deserialize)]
struct Chunk {
    message: OllamaMsg,
    done: bool,
    #[serde(default)]
    prompt_eval_count: u32,
    #[serde(default)]
    eval_count: u32,
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str { "ollama" }
    fn model(&self) -> &str { &self.model }

    async fn stream(
        &self,
        messages: &[super::Message],
        tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        let msgs: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                json!({
                    "role": match m.role {
                        super::Role::User => "user",
                        super::Role::Assistant => "assistant",
                        super::Role::System => "system",
                    },
                    "content": m.content.text(),
                })
            })
            .collect();

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&json!({
                "model": self.model,
                "messages": msgs,
                "stream": true,
                "tools": openai_tool_schemas(),
            }))
            .send()
            .await
            .context("connect to Ollama — is it running?")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamEvent::Error(format!("{status}: {text}"))).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut accumulated_text = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                    break;
                }
            };

            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find('\n') {
                let line = buf[..pos].to_string();
                buf = buf[pos + 1..].to_string();

                if line.is_empty() { continue; }
                let Ok(chunk) = serde_json::from_str::<Chunk>(&line) else { continue };

                if !chunk.message.content.is_empty() {
                    accumulated_text.push_str(&chunk.message.content);
                    let _ = tx.send(StreamEvent::TextDelta(chunk.message.content)).await;
                }

                if chunk.done {
                    if !chunk.message.tool_calls.is_empty() {
                        let calls: Vec<ToolCallData> = chunk
                            .message
                            .tool_calls
                            .into_iter()
                            .enumerate()
                            .map(|(i, tc)| ToolCallData {
                                id: format!("ollama-{i}"),
                                name: tc.function.name,
                                input: tc.function.arguments,
                            })
                            .collect();

                        let _ = tx
                            .send(StreamEvent::ToolCalls {
                                calls,
                                accumulated_text,
                                input_tokens: chunk.prompt_eval_count,
                                output_tokens: chunk.eval_count,
                            })
                            .await;
                    } else {
                        let _ = tx
                            .send(StreamEvent::Done {
                                input_tokens: chunk.prompt_eval_count,
                                output_tokens: chunk.eval_count,
                            })
                            .await;
                    }
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}
