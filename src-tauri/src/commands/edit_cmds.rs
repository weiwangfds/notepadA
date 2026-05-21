use serde::Serialize;
use tauri::State;

use crate::app_state::AppState;
use crate::viewport::manager::ViewportData;

/// Result returned after an edit operation.
#[derive(Debug, Clone, Serialize)]
pub struct EditResult {
    /// Updated viewport data.
    pub viewport: ViewportData,
    /// New cursor position (line, col).
    pub cursor_line: u64,
    pub cursor_col: u32,
    /// Whether the document is now dirty.
    pub dirty: bool,
    /// Whether undo is available.
    pub can_undo: bool,
    /// Whether redo is available.
    pub can_redo: bool,
}

/// Insert text at a specific line/column position.
#[tauri::command]
pub fn insert_text(
    tab_id: String,
    line: u64,
    col: u32,
    text: String,
    state: State<'_, AppState>,
) -> Result<EditResult, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();

    // Convert line/col to byte offset
    let bytes = d.content_bytes();
    let byte_pos = line_col_to_byte_offset(&bytes, line, col)?;

    // Insert into piece table
    d.piece_table.insert(byte_pos, text.as_bytes());
    d.rebuild_after_edit();

    // Calculate new cursor position
    let new_col = col + text.len() as u32;
    let bytes = d.content_bytes();
    let viewport = d.viewport.get_viewport(&bytes, line.saturating_sub(25), 50);

    Ok(EditResult {
        viewport,
        cursor_line: line,
        cursor_col: new_col,
        dirty: d.dirty,
        can_undo: true,
        can_redo: false,
    })
}

/// Delete a range of text from start_line:start_col to end_line:end_col.
#[tauri::command]
pub fn delete_range(
    tab_id: String,
    start_line: u64,
    start_col: u32,
    end_line: u64,
    end_col: u32,
    state: State<'_, AppState>,
) -> Result<EditResult, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();

    let bytes = d.content_bytes();
    let start_pos = line_col_to_byte_offset(&bytes, start_line, start_col)?;
    let end_pos = line_col_to_byte_offset(&bytes, end_line, end_col)?;

    if end_pos <= start_pos {
        return Err("Invalid range: end <= start".to_string());
    }

    let len = end_pos - start_pos;
    d.piece_table.delete(start_pos, len);
    d.rebuild_after_edit();

    let bytes = d.content_bytes();
    let viewport = d.viewport.get_viewport(&bytes, start_line.saturating_sub(25), 50);

    Ok(EditResult {
        viewport,
        cursor_line: start_line,
        cursor_col: start_col,
        dirty: d.dirty,
        can_undo: d.piece_table.is_dirty(),
        can_redo: false,
    })
}

/// Replace a range of text with new text.
#[tauri::command]
pub fn replace_range(
    tab_id: String,
    start_line: u64,
    start_col: u32,
    end_line: u64,
    end_col: u32,
    text: String,
    state: State<'_, AppState>,
) -> Result<EditResult, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();

    let bytes = d.content_bytes();
    let start_pos = line_col_to_byte_offset(&bytes, start_line, start_col)?;
    let end_pos = line_col_to_byte_offset(&bytes, end_line, end_col)?;

    if end_pos <= start_pos {
        return Err("Invalid range: end <= start".to_string());
    }

    let len = end_pos - start_pos;
    d.piece_table.replace(start_pos, len, text.as_bytes());
    d.rebuild_after_edit();

    // New cursor: after the replacement text on the start line
    let new_col = start_col + text.len() as u32;
    let bytes = d.content_bytes();
    let viewport = d.viewport.get_viewport(&bytes, start_line.saturating_sub(25), 50);

    Ok(EditResult {
        viewport,
        cursor_line: start_line,
        cursor_col: new_col,
        dirty: d.dirty,
        can_undo: d.piece_table.is_dirty(),
        can_redo: false,
    })
}

/// Undo the last edit operation.
#[tauri::command]
pub fn undo(
    tab_id: String,
    current_line: u64,
    state: State<'_, AppState>,
) -> Result<EditResult, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();

    if !d.piece_table.undo() {
        let bytes = d.content_bytes();
        let viewport = d.viewport.get_viewport(&bytes, current_line.saturating_sub(25), 50);
        return Ok(EditResult {
            viewport,
            cursor_line: current_line,
            cursor_col: 0,
            dirty: d.dirty,
            can_undo: false,
            can_redo: true,
        });
    }

    d.dirty = d.piece_table.is_dirty();
    let bytes = d.content_bytes();
    d.viewport.rebuild_line_index(&bytes);

    let viewport = d.viewport.get_viewport(&bytes, current_line.saturating_sub(25), 50);

    Ok(EditResult {
        viewport,
        cursor_line: current_line,
        cursor_col: 0,
        dirty: d.dirty,
        can_undo: d.piece_table.is_dirty(),
        can_redo: true,
    })
}

/// Redo the last undone operation.
#[tauri::command]
pub fn redo(
    tab_id: String,
    current_line: u64,
    state: State<'_, AppState>,
) -> Result<EditResult, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();

    if !d.piece_table.redo() {
        let bytes = d.content_bytes();
        let viewport = d.viewport.get_viewport(&bytes, current_line.saturating_sub(25), 50);
        return Ok(EditResult {
            viewport,
            cursor_line: current_line,
            cursor_col: 0,
            dirty: d.dirty,
            can_undo: true,
            can_redo: false,
        });
    }

    d.dirty = d.piece_table.is_dirty();
    let bytes = d.content_bytes();
    d.viewport.rebuild_line_index(&bytes);

    let viewport = d.viewport.get_viewport(&bytes, current_line.saturating_sub(25), 50);

    Ok(EditResult {
        viewport,
        cursor_line: current_line,
        cursor_col: 0,
        dirty: d.dirty,
        can_undo: true,
        can_redo: d.piece_table.is_dirty(),
    })
}

/// Convert a (line, col) position to a byte offset in the UTF-8 content.
/// Line and col are 0-based. Col is in characters (not bytes).
fn line_col_to_byte_offset(content: &[u8], line: u64, col: u32) -> Result<u64, String> {
    let mut current_line = 0u64;
    let mut offset = 0usize;

    while offset < content.len() && current_line < line {
        if content[offset] == b'\n' {
            current_line += 1;
            offset += 1;
        } else if content[offset] == b'\r' && offset + 1 < content.len() && content[offset + 1] == b'\n' {
            current_line += 1;
            offset += 2;
        } else {
            offset += 1;
        }
    }

    if current_line != line {
        return Err(format!("Line {} out of range (total {} lines)", line, current_line));
    }

    // Now advance `col` characters (not bytes) from current offset
    let mut chars_advanced = 0u32;
    while offset < content.len() && chars_advanced < col {
        // Skip one UTF-8 character
        let ch_len = if offset < content.len() {
            match content[offset] {
                0x00..=0x7F => 1,
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF7 => 4,
                _ => 1,
            }
        } else {
            break;
        };
        offset += ch_len;
        chars_advanced += 1;
    }

    Ok(offset as u64)
}
