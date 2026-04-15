use super::{gemini_tool_schemas, Block, Content, Provider, Role, StreamEvent, ToolCallData};
use anyhow::Context;
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::mpsc;

pub struct GeminiProvider {
    key: String,
    model: String,
    client: Client,
}

impl GeminiProvider {
    pub fn new(key: String, model: String) -> Self {
        Self { key, model, client: Client::new() }
    }
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default, rename = "usageMetadata")]
    usage: Option<UsageMeta>,
}

#[derive(Deserialize)]
struct Candidate {
    content: GeminiContent,
    #[serde(default, rename = "finishReason")]
    finish_reason: String,
}

#[derive(Deserialize)]
struct GeminiContent {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Part {
    Text { text: String },
    FunctionCall { #[serde(rename = "functionCall")] function_call: FunctionCall },
    FunctionResponse { #[serde(rename = "functionResponse")] function_response: serde_json::Value },
    Other(serde_json::Value),
}

#[derive(Deserialize)]
struct FunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Deserialize)]
struct UsageMeta {
    #[serde(default, rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(default, rename = "candidatesTokenCount")]
    candidates_token_count: u32,
}

fn to_gemini_messages(messages: &[super::Message]) -> (Option<String>, Vec<serde_json::Value>) {
    let mut system_text: Option<String> = None;
    let mut contents: Vec<serde_json::Value> = Vec::new();

    for msg in messages {
        match &msg.role {
            Role::System => {
                system_text = Some(msg.content.text().to_string());
            }
            Role::User => {
                if let Content::Blocks(blocks) = &msg.content {
                    let all_tool = blocks.iter().all(|b| matches!(b, Block::ToolResult { .. }));
                    if all_tool {
                        let parts: Vec<serde_json::Value> = blocks
                            .iter()
                            .filter_map(|b| {
                                if let Block::ToolResult { tool_use_id, content } = b {
                                    Some(serde_json::json!({
                                        "functionResponse": {
                                            "name": tool_use_id,
                                            "response": { "content": content }
                                        }
                                    }))
                                } else {
                                    None
                                }
                            })
                            .collect();
                        contents.push(serde_json::json!({ "role": "user", "parts": parts }));
                        continue;
                    }
                }
                contents.push(serde_json::json!({
                    "role": "user",
                    "parts": [{ "text": msg.content.text() }]
                }));
            }
            Role::Assistant => {
                if let Content::Blocks(blocks) = &msg.content {
                    let parts: Vec<serde_json::Value> = blocks
                        .iter()
                        .filter_map(|b| match b {
                            Block::Text { text } if !text.is_empty() => {
                                Some(serde_json::json!({ "text": text }))
                            }
                            Block::ToolUse { id, name, input } => {
                                let _ = id;
                                Some(serde_json::json!({
                                    "functionCall": { "name": name, "args": input }
                                }))
                            }
                            _ => None,
                        })
                        .collect();
                    contents.push(serde_json::json!({ "role": "model", "parts": parts }));
                } else {
                    contents.push(serde_json::json!({
                        "role": "model",
                        "parts": [{ "text": msg.content.text() }]
                    }));
                }
            }
        }
    }

    (system_text, contents)
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str { "gemini" }
    fn model(&self) -> &str { &self.model }

    async fn stream(
        &self,
        messages: &[super::Message],
        tx: mpsc::Sender<StreamEvent>,
    ) -> anyhow::Result<()> {
        let (system_text, contents) = to_gemini_messages(messages);

        let mut body = serde_json::json!({
            "contents": contents,
            "tools": gemini_tool_schemas(),
            "generationConfig": { "maxOutputTokens": 8192 },
        });

        if let Some(sys) = system_text {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": sys }]
            });
        }

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse&key={}",
            self.model, self.key
        );

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("connect to Gemini")?;

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
        let mut tool_calls: Vec<ToolCallData> = Vec::new();
        let mut has_tool_calls = false;

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
                    let Ok(resp) = serde_json::from_str::<Response>(data) else { continue };

                    if let Some(u) = resp.usage {
                        input_tokens = u.prompt_token_count;
                        output_tokens = u.candidates_token_count;
                    }

                    for candidate in resp.candidates {
                        let _ = candidate.finish_reason;
                        for part in candidate.content.parts {
                            match part {
                                Part::Text { text } => {
                                    if !text.is_empty() {
                                        accumulated_text.push_str(&text);
                                        let _ = tx.send(StreamEvent::TextDelta(text)).await;
                                    }
                                }
                                Part::FunctionCall { function_call } => {
                                    has_tool_calls = true;
                                    tool_calls.push(ToolCallData {
                                        id: format!("gemini-{}", tool_calls.len()),
                                        name: function_call.name,
                                        input: function_call.args,
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        if has_tool_calls && !tool_calls.is_empty() {
            let _ = tx
                .send(StreamEvent::ToolCalls {
                    calls: tool_calls,
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
