use super::Tool;
use anyhow::{Context, Result};
use serde_json::{json, Value};

pub struct ReadFile;

impl Tool for ReadFile {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read the full contents of a file." }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to the file" }
            },
            "required": ["path"]
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"].as_str().context("missing path")?;
        std::fs::read_to_string(path).context(format!("read {path}"))
    }
}

pub struct WriteFile;

impl Tool for WriteFile {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file, creating parent dirs as needed." }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Destination path" },
                "content": { "type": "string", "description": "Content to write" }
            },
            "required": ["path", "content"]
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let path    = input["path"].as_str().context("missing path")?;
        let content = input["content"].as_str().context("missing content")?;
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, content).context(format!("write {path}"))?;
        Ok(format!("wrote {} bytes to {path}", content.len()))
    }
}
