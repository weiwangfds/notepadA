use crate::buffer::line_index::LineIndex;
use crate::file::encoding::{convert_to_utf8, detect, EncodingInfo, LineEnding};

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
pub struct ViewportManager {
    /// The line index for this document.
    line_index: LineIndex,
    /// Encoding info.
    encoding_info: EncodingInfo,
    /// File size in bytes.
    file_size: u64,
    /// File name for display.
    file_name: String,
    /// The converted UTF-8 text (only for small files that fit in memory).
    /// For large files, we read from the mmap on demand.
    utf8_text: Option<String>,
    /// BOM length in bytes (for future save support).
    #[allow(dead_code)]
    bom_len: usize,
}

impl ViewportManager {
    /// Create a new viewport manager for a document.
    /// `raw_bytes` is the mmap'd file content.
    /// `file_name` is the display name of the file.
    pub fn new(raw_bytes: &[u8], file_size: u64, file_name: String) -> Self {
        // Detect encoding
        let encoding_info = detect(raw_bytes, 64 * 1024); // Scan first 64KB for detection

        // Detect BOM length
        let bom_len = detect_bom_len(raw_bytes);

        // For Phase 1, convert the entire file to UTF-8 upfront.
        // For very large files, this will be replaced with on-demand conversion
        // in the Piece Table phase.
        let utf8_text = convert_to_utf8(raw_bytes, &encoding_info.encoding, bom_len);

        // Build line index on the UTF-8 text
        let line_index = LineIndex::new(utf8_text.as_bytes(), utf8_text.len() as u64);

        // Build full index for now (Phase 1 simplicity)
        let mut line_index = line_index;
        line_index.build_full(utf8_text.as_bytes());

        Self {
            line_index,
            encoding_info,
            file_size,
            file_name,
            utf8_text: Some(utf8_text),
            bom_len,
        }
    }

    /// Get a viewport of lines starting from `start_line`.
    pub fn get_viewport(&self, start_line: u64, line_count: u32) -> ViewportData {
        let lines = if let Some(text) = &self.utf8_text {
            self.line_index.get_lines(text.as_bytes(), start_line, line_count)
        } else {
            Vec::new()
        };

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
}

fn detect_bom_len(bytes: &[u8]) -> usize {
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
