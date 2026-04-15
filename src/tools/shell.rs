use super::Tool;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct Bash;

impl Tool for Bash {
    fn name(&self) -> &str { "bash" }
    fn description(&self) -> &str { "Execute a bash command and return stdout + stderr." }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The bash command to run" }
            },
            "required": ["command"]
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let command = input["command"].as_str().context("missing command")?;
        let out = Command::new("bash")
            .arg("-c")
            .arg(command)
            .output()
            .context("spawn bash")?;

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);

        let mut result = stdout.to_string();
        if !stderr.is_empty() {
            if !result.is_empty() { result.push('\n'); }
            result.push_str("stderr: ");
            result.push_str(&stderr);
        }
        if result.is_empty() {
            result = format!("exit {}", out.status.code().unwrap_or(-1));
        }
        Ok(result)
    }
}
