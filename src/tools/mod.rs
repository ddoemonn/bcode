pub mod fs;
pub mod shell;

use anyhow::Result;
use serde_json::Value;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> Value;
    fn execute(&self, input: Value) -> Result<String>;
}

pub fn registry() -> Vec<Box<dyn Tool>> {
    vec![
        Box::new(fs::ReadFile),
        Box::new(fs::WriteFile),
        Box::new(fs::ReplaceInFile),
        Box::new(fs::ListDir),
        Box::new(fs::Glob),
        Box::new(shell::Bash),
    ]
}

pub fn schemas() -> Vec<Value> {
    registry()
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name(),
                "description": t.description(),
                "input_schema": t.schema(),
            })
        })
        .collect()
}

pub fn execute(name: &str, input: Value) -> Result<String> {
    registry()
        .iter()
        .find(|t| t.name() == name)
        .ok_or_else(|| anyhow::anyhow!("unknown tool: {name}"))
        .and_then(|t| t.execute(input))
}
