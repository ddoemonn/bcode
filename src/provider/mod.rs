pub mod anthropic;
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
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
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

    pub fn assistant_with_tools(text: String, calls: &[ToolCallData]) -> Self {
        let mut blocks = Vec::new();
        if !text.is_empty() {
            blocks.push(Block::Text { text });
        }
        for c in calls {
            blocks.push(Block::ToolUse { id: c.id.clone(), name: c.name.clone(), input: c.input.clone() });
        }
        Self { role: Role::Assistant, content: Content::Blocks(blocks) }
    }

    pub fn tool_results(results: &[(String, String)]) -> Self {
        let blocks = results
            .iter()
            .map(|(id, output)| Block::ToolResult { tool_use_id: id.clone(), content: output.clone() })
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
