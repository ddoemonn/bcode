use super::Tool;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::process::Command;

pub struct Bash;

impl Tool for Bash {
    fn name(&self) -> &str { "bash" }

    fn description(&self) -> &str {
        "Execute a bash command and return stdout + stderr. Runs with a 30-second timeout."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to run"
                },
                "timeout_secs": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 30, max: 120)"
                }
            },
            "required": ["command"]
        })
    }

    fn execute(&self, input: Value) -> Result<String> {
        let command = input["command"].as_str().context("missing command")?;
        let timeout = input["timeout_secs"]
            .as_u64()
            .unwrap_or(30)
            .min(120);

        let child = Command::new("bash")
            .arg("-c")
            .arg(command)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("spawn bash")?;

        let result = std::thread::scope(|s| {
            let handle = s.spawn(|| child.wait_with_output());

            std::thread::sleep(std::time::Duration::from_secs(timeout));
            handle.join().unwrap_or_else(|_| {
                Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "timeout"))
            })
        });

        let out = match result {
            Ok(output) => output,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                return Ok(format!("command timed out after {timeout}s"));
            }
            Err(e) => return Err(e.into()),
        };

        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);

        let mut result_str = stdout.to_string();
        if !stderr.is_empty() {
            if !result_str.is_empty() { result_str.push('\n'); }
            result_str.push_str("stderr: ");
            result_str.push_str(&stderr);
        }
        if result_str.is_empty() {
            result_str = format!("exit {}", out.status.code().unwrap_or(-1));
        }
        if !out.status.success() && !out.stderr.is_empty() {
            result_str.push_str(&format!("\nexit code: {}", out.status.code().unwrap_or(-1)));
        }

        Ok(result_str)
    }
}
