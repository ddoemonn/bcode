use anyhow::Result;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub turn: usize,
    pub files: Vec<FileSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSnapshot {
    pub path: String,
    pub content: Option<String>,
}

fn checkpoints_dir(session_id: &str) -> Option<PathBuf> {
    ProjectDirs::from("", "", "bcode")
        .map(|d| d.data_dir().join("checkpoints").join(session_id))
}

pub fn snapshot(session_id: &str, turn: usize, paths: &[&str]) -> Result<Checkpoint> {
    let files = paths
        .iter()
        .map(|p| FileSnapshot {
            path: p.to_string(),
            content: std::fs::read_to_string(p).ok(),
        })
        .collect();

    let cp = Checkpoint { turn, files };

    if let Some(dir) = checkpoints_dir(session_id) {
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{turn}.json")), serde_json::to_string(&cp)?)?;
    }

    Ok(cp)
}

pub fn restore(session_id: &str, turn: usize) -> Result<Checkpoint> {
    let dir = checkpoints_dir(session_id)
        .ok_or_else(|| anyhow::anyhow!("no data directory"))?;
    let cp: Checkpoint =
        serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{turn}.json")))?)?;

    for file in &cp.files {
        match &file.content {
            Some(content) => {
                if let Some(parent) = Path::new(&file.path).parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&file.path, content)?;
            }
            None => {
                let _ = std::fs::remove_file(&file.path);
            }
        }
    }

    Ok(cp)
}

pub fn list(session_id: &str) -> Vec<usize> {
    let Some(dir) = checkpoints_dir(session_id) else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };

    let mut turns: Vec<usize> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.strip_suffix(".json")?.parse().ok()
        })
        .collect();

    turns.sort();
    turns
}

pub fn latest(session_id: &str) -> Option<usize> {
    list(session_id).into_iter().last()
}
