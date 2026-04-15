use super::{openai_tool_schemas, to_openai_messages, Provider, StreamEvent, ToolCallData};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use tokio::sync::mpsc;

pub struct OpenAIProvider {
    key: String,
    model: String,
    base_url: String,
    client: Client,
}

impl OpenAIProvider {
    pub fn new(key: String, model: String, base_url: Option<String>) -> Self {
        Self {
            key,
            model,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com".to_string()),
            client: Client::new(),
        }
    }
}

#[derive(Deserialize, Default)]
struct Delta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize)]
struct ToolCallDelta {
    index: usize,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<FunctionDelta>,
}

#[derive(Deserialize)]
struct FunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct Choice {
    delta: Delta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Chunk {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

struct ToolState {
    id: String,
    name: String,
    args_buf: String,
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str { "openai" }
    fn model(&self) -> &str { &self.model }

    async fn stream(
        &self,
        messages: &[super::Message],
        tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        let msgs = to_openai_messages(messages);

        let body = json!({
            "model": self.model,
            "messages": msgs,
            "stream": true,
            "stream_options": { "include_usage": true },
            "tools": openai_tool_schemas(),
        });

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .bearer_auth(&self.key)
            .json(&body)
            .send()
            .await
            .context("connect to OpenAI")?;

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
        let mut accumulated_text = String::new();
        let mut tool_states: BTreeMap<usize, ToolState> = BTreeMap::new();
        let mut finish_reason = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(StreamEvent::Error(e.to_string())).await;
                    break;
                }
            };

            buf.push_str(&String::from_utf8_lossy(&chunk));

            while let Some(pos) = buf.find("\n\n") {
                let block = buf[..pos].to_string();
                buf = buf[pos + 2..].to_string();

                for line in block.lines() {
                    let Some(data) = line.strip_prefix("data: ") else { continue };
                    if data == "[DONE]" {
                        continue;
                    }
                    let Ok(c) = serde_json::from_str::<Chunk>(data) else { continue };

                    if let Some(u) = c.usage {
                        input_tokens = u.prompt_tokens;
                        output_tokens = u.completion_tokens;
                    }

                    for choice in c.choices {
                        if let Some(fr) = choice.finish_reason {
                            if !fr.is_empty() {
                                finish_reason = fr;
                            }
                        }

                        if let Some(text) = choice.delta.content {
                            if !text.is_empty() {
                                accumulated_text.push_str(&text);
                                let _ = tx.send(StreamEvent::TextDelta(text)).await;
                            }
                        }

                        if let Some(tc_deltas) = choice.delta.tool_calls {
                            for tcd in tc_deltas {
                                let state = tool_states.entry(tcd.index).or_insert_with(|| {
                                    ToolState {
                                        id: String::new(),
                                        name: String::new(),
                                        args_buf: String::new(),
                                    }
                                });
                                if let Some(id) = tcd.id {
                                    state.id = id;
                                }
                                if let Some(func) = tcd.function {
                                    if let Some(name) = func.name {
                                        state.name = name;
                                    }
                                    if let Some(args) = func.arguments {
                                        state.args_buf.push_str(&args);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if finish_reason == "tool_calls" && !tool_states.is_empty() {
            let calls: Vec<ToolCallData> = tool_states
                .into_values()
                .map(|s| {
                    let input = serde_json::from_str(&s.args_buf)
                        .unwrap_or(serde_json::Value::Object(Default::default()));
                    ToolCallData { id: s.id, name: s.name, input }
                })
                .collect();

            let _ = tx
                .send(StreamEvent::ToolCalls {
                    calls,
                    accumulated_text,
                    input_tokens,
                    output_tokens,
                })
                .await;
        } else {
            let _ = tx.send(StreamEvent::Done { input_tokens, output_tokens }).await;
        }

        Ok(())
    }
}
