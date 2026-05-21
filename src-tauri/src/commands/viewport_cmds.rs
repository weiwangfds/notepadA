use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_state::AppState;
use crate::viewport::manager::ViewportData;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ViewportRequest {
    pub tab_id: String,
    pub start_line: u64,
    pub line_count: u32,
}

#[tauri::command]
pub fn get_viewport(
    tab_id: String,
    start_line: u64,
    line_count: u32,
    state: State<'_, AppState>,
) -> Result<ViewportData, String> {
    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();
    let bytes = d.content_bytes();
    Ok(d.viewport.get_viewport(&bytes, start_line, line_count))
}

#[tauri::command]
pub fn goto_line(
    tab_id: String,
    line: u64,
    state: State<'_, AppState>,
) -> Result<ViewportData, String> {
    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();
    let bytes = d.content_bytes();
    Ok(d.viewport.get_viewport(&bytes, line, 50))
}

#[tauri::command]
pub fn get_line_count(tab_id: String, state: State<'_, AppState>) -> Result<LineCountInfo, String> {
    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();
    let bytes = d.content_bytes();
    let vp = d.viewport.get_viewport(&bytes, 0, 0);
    Ok(LineCountInfo {
        total_lines: d.viewport.total_lines(),
        file_size: d.viewport.file_size(),
        index_progress: vp.index_progress,
        index_complete: vp.index_complete,
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct LineCountInfo {
    pub total_lines: u64,
    pub file_size: u64,
    pub index_progress: f32,
    pub index_complete: bool,
}
