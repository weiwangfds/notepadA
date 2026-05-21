use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::file::mapper::FileMapper;
use crate::viewport::manager::ViewportManager;

/// Represents an open document (tab).
pub struct Document {
    /// Unique tab ID.
    pub id: String,
    /// File path on disk.
    pub path: PathBuf,
    /// The mmap'd file reader. Kept alive so the mmap mapping persists.
    #[allow(dead_code)]
    pub mapper: FileMapper,
    /// The viewport manager for this document.
    pub viewport: ViewportManager,
    /// Whether the document has unsaved edits.
    pub dirty: bool,
}

/// Global application state shared across all Tauri commands.
pub struct AppState {
    /// Open documents keyed by tab ID.
    pub docs: RwLock<HashMap<String, Arc<RwLock<Document>>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            docs: RwLock::new(HashMap::new()),
        }
    }

    /// Open a file and add it as a new tab.
    /// Returns the tab ID.
    pub fn open_file(&self, path: &std::path::Path) -> Result<String, String> {
        let mapper = FileMapper::open(path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("untitled")
            .to_string();

        let raw_bytes = mapper.as_bytes();
        let file_size = mapper.file_size();

        let viewport = ViewportManager::new(raw_bytes, file_size, file_name.clone());

        let tab_id = Uuid::new_v4().to_string();

        let doc = Document {
            id: tab_id.clone(),
            path: path.to_path_buf(),
            mapper,
            viewport,
            dirty: false,
        };

        let mut docs = self.docs.write();
        docs.insert(tab_id.clone(), Arc::new(RwLock::new(doc)));

        Ok(tab_id)
    }

    /// Close a tab by ID.
    pub fn close_file(&self, tab_id: &str) -> Result<(), String> {
        let mut docs = self.docs.write();
        docs.remove(tab_id)
            .ok_or_else(|| format!("Tab not found: {}", tab_id))?;
        Ok(())
    }

    /// Get a document by tab ID.
    pub fn get_doc(&self, tab_id: &str) -> Result<Arc<RwLock<Document>>, String> {
        let docs = self.docs.read();
        docs.get(tab_id)
            .cloned()
            .ok_or_else(|| format!("Tab not found: {}", tab_id))
    }

    /// Get all open tab infos.
    pub fn get_tabs(&self) -> Vec<TabInfo> {
        let docs = self.docs.read();
        docs.values()
            .map(|doc| {
                let d = doc.read();
                TabInfo {
                    id: d.id.clone(),
                    file_name: d.viewport.file_name(),
                    path: d.path.to_string_lossy().to_string(),
                    dirty: d.dirty,
                }
            })
            .collect()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TabInfo {
    pub id: String,
    pub file_name: String,
    pub path: String,
    pub dirty: bool,
}
