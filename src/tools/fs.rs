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
                "path": { "type": "string" }
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
                "path":    { "type": "string" },
                "content": { "type": "string" }
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

pub struct ReplaceInFile;

impl Tool for ReplaceInFile {
    fn name(&self) -> &str { "replace_in_file" }
    fn description(&self) -> &str {
        "Replace an exact string in a file. Fails if the string is not found or is ambiguous (multiple occurrences). Prefer this over write_file for targeted edits."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":       { "type": "string", "description": "File to edit" },
                "old_string": { "type": "string", "description": "Exact text to find" },
                "new_string": { "type": "string", "description": "Replacement text" }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let path       = input["path"].as_str().context("missing path")?;
        let old_string = input["old_string"].as_str().context("missing old_string")?;
        let new_string = input["new_string"].as_str().context("missing new_string")?;

        let content = std::fs::read_to_string(path).context(format!("read {path}"))?;
        let count = content.matches(old_string).count();

        if count == 0 {
            anyhow::bail!("old_string not found in {path}");
        }
        if count > 1 {
            anyhow::bail!("old_string is ambiguous ({count} occurrences in {path}); add more surrounding context");
        }

        let new_content = content.replacen(old_string, new_string, 1);
        std::fs::write(path, &new_content).context(format!("write {path}"))?;

        Ok(format!(
            "replaced in {path}\n\n{}\n{}",
            old_string.lines().map(|l| format!("- {l}")).collect::<Vec<_>>().join("\n"),
            new_string.lines().map(|l| format!("+ {l}")).collect::<Vec<_>>().join("\n"),
        ))
    }
}

pub struct ListDir;

impl Tool for ListDir {
    fn name(&self) -> &str { "list_dir" }
    fn description(&self) -> &str { "List files and directories at a path (one level deep)." }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory to list (default: .)" }
            },
            "required": []
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let path = input["path"].as_str().unwrap_or(".");
        let mut entries = std::fs::read_dir(path)
            .context(format!("list {path}"))?
            .filter_map(|e| e.ok())
            .collect::<Vec<_>>();

        entries.sort_by_key(|e| {
            let is_file = e.file_type().map(|t| t.is_file()).unwrap_or(false);
            (is_file, e.file_name())
        });

        let lines: Vec<String> = entries
            .iter()
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let meta = e.metadata().ok();
                if meta.as_ref().map(|m| m.is_dir()).unwrap_or(false) {
                    format!("{name}/")
                } else {
                    let size = meta.map(|m| fmt_size(m.len())).unwrap_or_default();
                    format!("{name}  {size}")
                }
            })
            .collect();

        Ok(lines.join("\n"))
    }
}

pub struct Glob;

impl Tool for Glob {
    fn name(&self) -> &str { "glob" }
    fn description(&self) -> &str {
        "Find files matching a glob pattern (e.g. **/*.rs, src/**/*.toml)."
    }
    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "Glob pattern" }
            },
            "required": ["pattern"]
        })
    }
    fn execute(&self, input: Value) -> Result<String> {
        let pattern = input["pattern"].as_str().context("missing pattern")?;
        let paths: Vec<String> = glob::glob(pattern)
            .context("invalid pattern")?
            .filter_map(|e| e.ok())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if paths.is_empty() {
            return Ok("no matches".to_string());
        }
        Ok(paths.join("\n"))
    }
}

fn fmt_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1_024 {
        format!("{:.1}K", bytes as f64 / 1_024.0)
    } else {
        format!("{bytes}B")
    }
}
