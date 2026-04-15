use super::{Message, Provider, Role, StreamEvent, ToolCallData};
use crate::tools;
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
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
    MessageStart { message: MessageStartData },
    #[serde(rename = "content_block_start")]
    ContentBlockStart { index: usize, content_block: StartBlock },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: Delta },
    #[serde(rename = "message_delta")]
    MessageDelta { delta: MsgDelta, usage: MsgDeltaUsage },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct MessageStartData {
    usage: StartUsage,
}

#[derive(Deserialize)]
struct StartUsage {
    input_tokens: u32,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum StartBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String },
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Delta {
    #[serde(rename = "text_delta")]
    Text { text: String },
    #[serde(rename = "input_json_delta")]
    InputJson { partial_json: String },
}

#[derive(Deserialize)]
struct MsgDelta {
    stop_reason: String,
}

#[derive(Deserialize)]
struct MsgDeltaUsage {
    output_tokens: u32,
}

enum BlockState {
    Text,
    ToolUse { id: String, name: String, json_buf: String },
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str { "anthropic" }
    fn model(&self) -> &str { &self.model }

    async fn stream(&self, messages: &[Message], tx: mpsc::Sender<StreamEvent>) -> anyhow::Result<()> {
        let system = messages.iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.text().to_string());

        let msgs: Vec<&Message> = messages.iter()
            .filter(|m| m.role != Role::System)
            .collect();

        let mut body = json!({
            "model": self.model,
            "messages": msgs,
            "max_tokens": 8192,
            "stream": true,
            "tools": tools::schemas(),
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
            .context("connect to Anthropic")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            let _ = tx.send(StreamEvent::Error(format!("{status}: {text}"))).await;
            return Ok(());
        }

        let mut stream = response.bytes_stream();
        let mut buf = String::new();
        let mut input_tokens = 0u32;
        let mut accumulated_text = String::new();
        let mut blocks: BTreeMap<usize, BlockState> = BTreeMap::new();

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

                        Event::ContentBlockStart { index, content_block } => {
                            match content_block {
                                StartBlock::Text { .. } => {
                                    blocks.insert(index, BlockState::Text);
                                }
                                StartBlock::ToolUse { id, name } => {
                                    blocks.insert(index, BlockState::ToolUse {
                                        id, name, json_buf: String::new(),
                                    });
                                }
                            }
                        }

                        Event::ContentBlockDelta { index, delta } => match delta {
                            Delta::Text { text } => {
                                accumulated_text.push_str(&text);
                                let _ = tx.send(StreamEvent::TextDelta(text)).await;
                            }
                            Delta::InputJson { partial_json } => {
                                if let Some(BlockState::ToolUse { json_buf, .. }) = blocks.get_mut(&index) {
                                    json_buf.push_str(&partial_json);
                                }
                            }
                        },

                        Event::MessageDelta { delta, usage } => {
                            if delta.stop_reason == "tool_use" {
                                let calls: Vec<ToolCallData> = blocks
                                    .values()
                                    .filter_map(|b| {
                                        if let BlockState::ToolUse { id, name, json_buf } = b {
                                            let input = serde_json::from_str(json_buf)
                                                .unwrap_or(serde_json::Value::Object(Default::default()));
                                            Some(ToolCallData { id: id.clone(), name: name.clone(), input })
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();

                                let _ = tx.send(StreamEvent::ToolCalls {
                                    calls,
                                    accumulated_text: accumulated_text.clone(),
                                    input_tokens,
                                    output_tokens: usage.output_tokens,
                                }).await;
                            } else {
                                let _ = tx.send(StreamEvent::Done {
                                    input_tokens,
                                    output_tokens: usage.output_tokens,
                                }).await;
                            }
                        }

                        Event::Other => {}
                    }
                }
            }
        }

        Ok(())
    }
}
