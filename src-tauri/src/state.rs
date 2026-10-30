use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ItemStatus {
    Copying,
    Ready,
    Failed,
    Transforming,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShelfItem {
    pub id: String,
    pub name: String,
    pub extension: String,
    pub kind: String,
    pub size_bytes: u64,
    pub status: ItemStatus,
    pub progress: f32,
    pub error: Option<String>,
    /// Absolute path to the current active version (staged copy or transform output).
    pub active_path: PathBuf,
    /// Absolute path to the original staged input (never mutated by transforms).
    pub staged_input_path: PathBuf,
    pub original_source_path: Option<PathBuf>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Shelf {
    pub id: String,
    pub title: String,
    pub items: Vec<ShelfItem>,
    pub is_pinned: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Shelf {
    pub fn new(title: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            items: Vec::new(),
            is_pinned: false,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|i| i.size_bytes).sum()
    }

    pub fn ready_count(&self) -> usize {
        self.items
            .iter()
            .filter(|i| i.status == ItemStatus::Ready)
            .count()
    }
}

#[derive(Debug, Default)]
pub struct AppState {
    pub shelves: Mutex<HashMap<String, Shelf>>,
    pub active_shelf_id: Mutex<Option<String>>,
    pub data_root: Mutex<Option<PathBuf>>,
}

impl AppState {
    pub fn with_data_root(root: PathBuf) -> Self {
        Self {
            shelves: Mutex::new(HashMap::new()),
            active_shelf_id: Mutex::new(None),
            data_root: Mutex::new(Some(root)),
        }
    }
}
