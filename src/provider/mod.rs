pub mod anthropic;
pub mod gemini;
pub mod ollama;
pub mod openai;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

impl Content {
    pub fn text(&self) -> &str {
        match self {
            Content::Text(s) => s,
            Content::Blocks(blocks) => blocks
                .iter()
                .find_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None })
                .unwrap_or(""),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Block {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse { id: String, name: String, input: serde_json::Value },
    #[serde(rename = "tool_result")]
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Content,
}

impl Message {
    pub fn user(text: impl Into<String>) -> Self {
        Self { role: Role::User, content: Content::Text(text.into()) }
    }

    pub fn assistant(text: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: Content::Text(text.into()) }
    }

    pub fn system(text: impl Into<String>) -> Self {
        Self { role: Role::System, content: Content::Text(text.into()) }
    }

    pub fn assistant_with_tools(text: String, calls: &[ToolCallData]) -> Self {
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(Block::Text { text });
        }
        for c in calls {
            blocks.push(Block::ToolUse {
                id: c.id.clone(),
                name: c.name.clone(),
                input: c.input.clone(),
            });
        }
        Self { role: Role::Assistant, content: Content::Blocks(blocks) }
    }

    pub fn tool_results(results: &[(String, String)]) -> Self {
        let blocks = results
            .iter()
            .map(|(id, output)| Block::ToolResult {
                tool_use_id: id.clone(),
                content: output.clone(),
            })
            .collect();
        Self { role: Role::User, content: Content::Blocks(blocks) }
    }
}

#[derive(Debug, Clone)]
pub struct ToolCallData {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

#[derive(Debug)]
pub enum StreamEvent {
    TextDelta(String),
    ToolCalls {
        calls: Vec<ToolCallData>,
        accumulated_text: String,
        input_tokens: u32,
        output_tokens: u32,
    },
    ToolResult {
        id: String,
        output: String,
    },
    Done {
        input_tokens: u32,
        output_tokens: u32,
    },
    Error(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &str;
    fn model(&self) -> &str;
    async fn stream(&self, messages: &[Message], tx: mpsc::Sender<StreamEvent>) -> anyhow::Result<()>;
}

pub fn openai_tool_schemas() -> Vec<serde_json::Value> {
    crate::tools::registry()
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name(),
                    "description": t.description(),
                    "parameters": t.schema(),
                }
            })
        })
        .collect()
}

pub fn gemini_tool_schemas() -> serde_json::Value {
    let decls: Vec<serde_json::Value> = crate::tools::registry()
        .iter()
        .map(|t| {
            let schema = t.schema();
            let properties = schema.get("properties").cloned().unwrap_or_default();
            let required = schema.get("required").cloned().unwrap_or_default();
            let props_gemini = convert_schema_types(&properties);
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "parameters": {
                    "type": "OBJECT",
                    "properties": props_gemini,
                    "required": required,
                }
            })
        })
        .collect();

    serde_json::json!([{ "functionDeclarations": decls }])
}

fn convert_schema_types(props: &serde_json::Value) -> serde_json::Value {
    if let Some(obj) = props.as_object() {
        let converted: serde_json::Map<String, serde_json::Value> = obj
            .iter()
            .map(|(k, v)| {
                let new_v = if let Some(t) = v.get("type").and_then(|t| t.as_str()) {
                    let gemini_type = match t {
                        "string" => "STRING",
                        "integer" | "number" => "NUMBER",
                        "boolean" => "BOOLEAN",
                        "array" => "ARRAY",
                        "object" => "OBJECT",
                        _ => "STRING",
                    };
                    let mut new = v.clone();
                    new["type"] = serde_json::json!(gemini_type);
                    new
                } else {
                    v.clone()
                };
                (k.clone(), new_v)
            })
            .collect();
        serde_json::Value::Object(converted)
    } else {
        props.clone()
    }
}

pub fn to_openai_messages(messages: &[Message]) -> Vec<serde_json::Value> {
    let mut result = Vec::new();

    for msg in messages {
        match &msg.role {
            Role::System => {
                result.push(serde_json::json!({
                    "role": "system",
                    "content": msg.content.text()
                }));
            }
            Role::User => {
                if let Content::Blocks(blocks) = &msg.content {
                    let all_tool = blocks.iter().all(|b| matches!(b, Block::ToolResult { .. }));
                    if all_tool {
                        for block in blocks {
                            if let Block::ToolResult { tool_use_id, content } = block {
                                result.push(serde_json::json!({
                                    "role": "tool",
                                    "tool_call_id": tool_use_id,
                                    "content": content,
                                }));
                            }
                        }
                        continue;
                    }
                }
                result.push(serde_json::json!({
                    "role": "user",
                    "content": msg.content.text()
                }));
            }
            Role::Assistant => {
                if let Content::Blocks(blocks) = &msg.content {
                    let text: String = blocks
                        .iter()
                        .filter_map(|b| if let Block::Text { text } = b { Some(text.as_str()) } else { None })
                        .collect::<Vec<_>>()
                        .join("");

                    let tool_calls: Vec<serde_json::Value> = blocks
                        .iter()
                        .filter_map(|b| {
                            if let Block::ToolUse { id, name, input } = b {
                                Some(serde_json::json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }))
                            } else {
                                None
                            }
                        })
                        .collect();

                    let mut m = serde_json::json!({"role": "assistant"});
                    if !text.is_empty() {
                        m["content"] = serde_json::json!(text);
                    } else {
                        m["content"] = serde_json::Value::Null;
                    }
                    if !tool_calls.is_empty() {
                        m["tool_calls"] = serde_json::json!(tool_calls);
                    }
                    result.push(m);
                } else {
                    result.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg.content.text()
                    }));
                }
            }
        }
    }

    result
}
