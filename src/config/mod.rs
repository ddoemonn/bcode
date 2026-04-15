use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub provider: Option<String>,
    pub model: Option<String>,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    #[serde(default)]
    pub base_urls: HashMap<String, String>,
    #[serde(default)]
    pub max_messages: Option<usize>,
    #[serde(default)]
    pub always_allowed_tools: Vec<String>,
}

impl Config {
    pub fn load() -> Self {
        path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let p = path().ok_or_else(|| anyhow::anyhow!("no config directory"))?;
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }
}

fn path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "bcode").map(|d| d.config_dir().join("config.json"))
}
