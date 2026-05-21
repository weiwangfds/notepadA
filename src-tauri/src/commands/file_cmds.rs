use std::path::PathBuf;

use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::app_state::TabInfo;

#[derive(Debug, Clone, Serialize)]
pub struct OpenFileResult {
    pub tab_id: String,
    pub file_name: String,
    pub path: String,
    pub file_size: u64,
    pub total_lines: u64,
    pub encoding: String,
    pub has_bom: bool,
    pub line_ending: String,
}

#[tauri::command]
pub fn open_file(path: String, state: State<'_, AppState>) -> Result<OpenFileResult, String> {
    let path = PathBuf::from(&path);
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let tab_id = state.open_file(&path)?;

    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();

    // Get initial viewport to return encoding info
    let vp = d.viewport.get_viewport(0, 1);

    Ok(OpenFileResult {
        tab_id,
        file_name: vp.file_name,
        path: path.to_string_lossy().to_string(),
        file_size: vp.file_size,
        total_lines: vp.total_lines,
        encoding: vp.encoding,
        has_bom: vp.has_bom,
        line_ending: vp.line_ending,
    })
}

#[tauri::command]
pub fn close_file(tab_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.close_file(&tab_id)
}

#[tauri::command]
pub fn get_tabs(state: State<'_, AppState>) -> Vec<TabInfo> {
    state.get_tabs()
}
