use crate::provider::Message;
use anyhow::Result;
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub id: String,
    pub title: String,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
    pub tags: Vec<String>,
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn derive_title(messages: &[Message]) -> String {
    messages
        .iter()
        .find(|m| matches!(m.role, crate::provider::Role::User))
        .map(|m| {
            let t = m.content.text();
            if t.len() > 60 { format!("{}…", &t[..57]) } else { t.to_string() }
        })
        .unwrap_or_else(|| "untitled".to_string())
}

fn sessions_dir() -> Option<PathBuf> {
    ProjectDirs::from("", "", "bcode").map(|d| d.data_dir().join("sessions"))
}

pub fn save(session: &Session) -> Result<()> {
    let dir = sessions_dir().ok_or_else(|| anyhow::anyhow!("cannot find data directory"))?;
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", session.id));
    std::fs::write(path, serde_json::to_string_pretty(session)?)?;
    Ok(())
}

pub fn load(id: &str) -> Result<Session> {
    let dir = sessions_dir().ok_or_else(|| anyhow::anyhow!("cannot find data directory"))?;
    let json = std::fs::read_to_string(dir.join(format!("{id}.json")))?;
    Ok(serde_json::from_str(&json)?)
}

pub fn tag(id: &str, tag: &str) -> Result<()> {
    let mut session = load(id)?;
    if !session.tags.contains(&tag.to_string()) {
        session.tags.push(tag.to_string());
        save(&session)?;
    }
    Ok(())
}

pub fn list() -> Vec<SessionMeta> {
    list_filtered("")
}

pub fn list_filtered(query: &str) -> Vec<SessionMeta> {
    let Some(dir) = sessions_dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };

    let query_lower = query.to_lowercase();

    let mut metas: Vec<SessionMeta> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map(|x| x == "json").unwrap_or(false))
        .filter_map(|e| {
            let json = std::fs::read_to_string(e.path()).ok()?;
            let s: Session = serde_json::from_str(&json).ok()?;
            if !query_lower.is_empty()
                && !s.title.to_lowercase().contains(&query_lower)
                && !s.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
            {
                return None;
            }
            Some(SessionMeta {
                id: s.id,
                title: s.title,
                updated_at: s.updated_at,
                message_count: s.messages.len(),
                tags: s.tags,
            })
        })
        .collect();

    metas.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    metas
}
