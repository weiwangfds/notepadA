use tauri::State;

use crate::app_state::AppState;
use crate::search::{regex_search, text_search, SearchMatch, SearchOptions};

/// Search for text in the document.
#[tauri::command]
pub fn search(
    tab_id: String,
    query: String,
    options: SearchOptions,
    state: State<'_, AppState>,
) -> Result<Vec<SearchMatch>, String> {
    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();
    let content = d.content_bytes();

    let results = if options.regex {
        regex_search::search_regex(&content, &query, &options)
    } else {
        text_search::search(&content, &query, &options)
    };

    Ok(results)
}

/// Find the next match after a given position.
#[tauri::command]
pub fn search_next(
    tab_id: String,
    query: String,
    options: SearchOptions,
    current_line: u64,
    current_col: u32,
    state: State<'_, AppState>,
) -> Result<Option<SearchMatch>, String> {
    let doc = state.get_doc(&tab_id)?;
    let d = doc.read();
    let content = d.content_bytes();

    let results = if options.regex {
        regex_search::search_regex(&content, &query, &options)
    } else {
        text_search::search(&content, &query, &options)
    };

    // Find the first match after the current position
    for m in &results {
        if m.line > current_line || (m.line == current_line && m.col > current_col) {
            return Ok(Some(m.clone()));
        }
    }

    // Wrap around: return first match
    Ok(results.into_iter().next())
}

/// Replace all occurrences of query with replacement text.
#[tauri::command]
pub fn replace_all(
    tab_id: String,
    query: String,
    replacement: String,
    options: SearchOptions,
    state: State<'_, AppState>,
) -> Result<u64, String> {
    let doc = state.get_doc(&tab_id)?;
    let mut d = doc.write();
    let content = d.content_bytes();

    let results = if options.regex {
        regex_search::search_regex(&content, &query, &options)
    } else {
        text_search::search(&content, &query, &options)
    };

    let count = results.len() as u64;

    // Replace in reverse order to maintain correct offsets
    for m in results.iter().rev() {
        let start = m.col as u64;
        let line_start_offset = line_to_byte_offset(&content, m.line);
        let abs_start = line_start_offset + start;
        let abs_end = abs_start + m.length as u64;

        d.piece_table.replace(abs_start, abs_end - abs_start, replacement.as_bytes());
    }

    if count > 0 {
        d.rebuild_after_edit();
    }

    Ok(count)
}

/// Convert a line number to a byte offset.
fn line_to_byte_offset(content: &[u8], line: u64) -> u64 {
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

    offset as u64
}
