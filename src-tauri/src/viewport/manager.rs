use crate::buffer::line_index::LineIndex;
use crate::file::encoding::{EncodingInfo, LineEnding};

/// Result of a viewport request.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ViewportData {
    /// The text lines in the viewport.
    pub lines: Vec<String>,
    /// The actual start line number (0-based).
    pub start_line: u64,
    /// Total number of lines in the file (may be approximate if indexing is in progress).
    pub total_lines: u64,
    /// Total file size in bytes.
    pub file_size: u64,
    /// File encoding info.
    pub encoding: String,
    /// Whether the file has a BOM.
    pub has_bom: bool,
    /// Line ending style.
    pub line_ending: String,
    /// Indexing progress 0.0..1.0.
    pub index_progress: f32,
    /// Whether indexing is complete.
    pub index_complete: bool,
    /// File name (for display).
    pub file_name: String,
}

/// Manages the viewport for a single open document.
///
/// In Phase 2, the PieceTable lives in Document and the text bytes are passed
/// to each method. This avoids lifetime issues with borrowing across structs.
pub struct ViewportManager {
    /// The line index for this document.
    line_index: LineIndex,
    /// Encoding info.
    encoding_info: EncodingInfo,
    /// File size in bytes.
    file_size: u64,
    /// File name for display.
    file_name: String,
    /// BOM length in bytes.
    bom_len: usize,
}

impl ViewportManager {
    /// Create a new viewport manager from pre-built components.
    pub fn new(
        line_index: LineIndex,
        encoding_info: EncodingInfo,
        file_size: u64,
        file_name: String,
        bom_len: usize,
    ) -> Self {
        Self {
            line_index,
            encoding_info,
            file_size,
            file_name,
            bom_len,
        }
    }

    /// Get a viewport of lines starting from `start_line`.
    /// `text_bytes` is the current UTF-8 content from PieceTable.
    pub fn get_viewport(&self, text_bytes: &[u8], start_line: u64, line_count: u32) -> ViewportData {
        let lines = self.line_index.get_lines(text_bytes, start_line, line_count);

        let line_ending_str = match self.encoding_info.line_ending {
            LineEnding::LF => "LF",
            LineEnding::CRLF => "CRLF",
            LineEnding::Mixed => "Mixed",
        };

        ViewportData {
            lines,
            start_line,
            total_lines: self.line_index.total_lines(),
            file_size: self.file_size,
            encoding: self.encoding_info.encoding.clone(),
            has_bom: self.encoding_info.has_bom,
            line_ending: line_ending_str.to_string(),
            index_progress: self.line_index.progress(),
            index_complete: self.line_index.is_complete(),
            file_name: self.file_name.clone(),
        }
    }

    /// Rebuild the line index from the given text bytes.
    /// Called after edits to keep the line index in sync.
    pub fn rebuild_line_index(&mut self, text_bytes: &[u8]) {
        self.line_index = LineIndex::new(text_bytes, text_bytes.len() as u64);
        self.line_index.build_full(text_bytes);
    }

    /// Get the total line count.
    pub fn total_lines(&self) -> u64 {
        self.line_index.total_lines()
    }

    /// Get the file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Get the file name for display.
    pub fn file_name(&self) -> String {
        self.file_name.clone()
    }

    /// Get the BOM length.
    pub fn bom_len(&self) -> usize {
        self.bom_len
    }

    /// Get the encoding info.
    pub fn encoding_info(&self) -> &EncodingInfo {
        &self.encoding_info
    }
}

pub fn detect_bom_len(bytes: &[u8]) -> usize {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return 3;
    }
    if bytes.len() >= 4 && bytes[0] == 0x00 && bytes[1] == 0x00 && bytes[2] == 0xFE && bytes[3] == 0xFF {
        return 4;
    }
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xFE && bytes[2] == 0x00 && bytes[3] == 0x00 {
        return 4;
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return 2;
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return 2;
    }
    0
}
