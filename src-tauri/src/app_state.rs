use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;

use crate::buffer::line_index::LineIndex;
use crate::buffer::piece_table::PieceTable;
use crate::file::encoding::{convert_to_utf8, detect};
use crate::file::mapper::FileMapper;
use crate::viewport::manager::{detect_bom_len, ViewportManager};

/// Represents an open document (tab).
pub struct Document {
    /// Unique tab ID.
    pub id: String,
    /// File path on disk.
    pub path: PathBuf,
    /// The mmap'd file reader. Kept alive so the mmap mapping persists.
    #[allow(dead_code)]
    pub mapper: FileMapper,
    /// Piece Table for editing support.
    pub piece_table: PieceTable,
    /// The viewport manager for this document.
    pub viewport: ViewportManager,
    /// Whether the document has unsaved edits.
    pub dirty: bool,
}

impl Document {
    /// Get the current UTF-8 content from the piece table.
    pub fn content_bytes(&self) -> Vec<u8> {
        self.piece_table.to_bytes()
    }

    /// Get the current content as a string.
    pub fn content_string(&self) -> String {
        self.piece_table.to_string_lossy()
    }

    /// After an edit, rebuild the line index to reflect the new content.
    pub fn rebuild_after_edit(&mut self) {
        let bytes = self.piece_table.to_bytes();
        self.viewport.rebuild_line_index(&bytes);
        self.dirty = true;
    }
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

        // Detect encoding
        let encoding_info = detect(raw_bytes, 64 * 1024);
        let bom_len = detect_bom_len(raw_bytes);

        // Convert to UTF-8
        let utf8_bytes = convert_to_utf8(raw_bytes, &encoding_info.encoding, bom_len).into_bytes();
        let content = Arc::new(utf8_bytes);

        // Build line index: initial scan first, then background for large files
        let line_index = LineIndex::new(&content, content.len() as u64);
        let line_index = Arc::new(line_index);
        if !line_index.is_complete() {
            line_index.build_background(Arc::clone(&content), None);
        }

        // Create piece table with the UTF-8 content
        let piece_table = PieceTable::new((*content).clone());

        // Create viewport manager
        let viewport = ViewportManager::new(
            line_index,
            encoding_info,
            file_size,
            file_name,
            bom_len,
        );

        let tab_id = Uuid::new_v4().to_string();

        let doc = Document {
            id: tab_id.clone(),
            path: path.to_path_buf(),
            mapper,
            piece_table,
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
