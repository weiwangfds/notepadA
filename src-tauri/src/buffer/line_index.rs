use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// Number of lines per sparse index group.
/// For a 100GB file with ~10 billion lines, this gives ~10M sparse entries (~80MB).
const GROUP_SIZE: u64 = 1024;

/// Maximum bytes to scan synchronously on file open for initial responsiveness.
const INITIAL_SCAN_BYTES: usize = 2 * 1024 * 1024; // 2MB

/// A two-level sparse line index for fast line-to-offset mapping.
pub struct LineIndex {
    /// Sparse index: maps group_number -> file offset of the first line in that group.
    /// group_number = line_number / GROUP_SIZE
    /// Stored sorted; can be binary-searched.
    sparse: Vec<(u64, u64)>,

    /// Total number of lines discovered so far.
    total_lines: AtomicU64,

    /// Bytes scanned so far.
    bytes_scanned: AtomicU64,

    /// Total file size.
    file_size: u64,

    /// Indexing progress as per-mille (0..1000).
    progress: AtomicU32,

    /// Whether full indexing is complete.
    complete: std::sync::atomic::AtomicBool,
}

impl LineIndex {
    /// Create a new line index by performing an initial scan of the file header.
    /// Returns immediately after scanning the first ~2MB.
    pub fn new(data: &[u8], file_size: u64) -> Self {
        let mut idx = Self {
            sparse: Vec::new(),
            total_lines: AtomicU64::new(0),
            bytes_scanned: AtomicU64::new(0),
            file_size,
            progress: AtomicU32::new(0),
            complete: std::sync::atomic::AtomicBool::new(false),
        };

        // Initial scan: first N bytes
        let scan_len = data.len().min(INITIAL_SCAN_BYTES);
        idx.scan_chunk(data, 0, scan_len);

        // If file fits in initial scan, mark complete
        if scan_len >= data.len() {
            // Add one more line for the last line (file may not end with newline)
            idx.complete.store(true, Ordering::Release);
            idx.progress.store(1000, Ordering::Release);
        }

        idx
    }

    /// Scan a chunk of data and update the index.
    fn scan_chunk(&mut self, data: &[u8], file_offset: u64, len: usize) {
        let chunk = &data[file_offset as usize..(file_offset as usize + len)];
        let mut line_count = self.total_lines.load(Ordering::Relaxed);
        let mut pos: usize = 0;

        // Record the first line of this chunk if it starts a new group
        let group = line_count / GROUP_SIZE;
        if self.sparse.last().map_or(true, |(g, _)| *g != group) {
            self.sparse.push((group, file_offset));
        }

        while pos < chunk.len() {
            // Find next newline
            match memchr::memchr(b'\n', &chunk[pos..]) {
                Some(nl_pos) => {
                    line_count += 1;
                    pos += nl_pos + 1;

                    // Record sparse entry if entering a new group
                    let group = line_count / GROUP_SIZE;
                    if self.sparse.last().map_or(true, |(g, _)| *g != group) {
                        let abs_offset = file_offset + pos as u64;
                        self.sparse.push((group, abs_offset));
                    }
                }
                None => {
                    // No more newlines in this chunk.
                    // If we scanned to the end of the file and there is trailing
                    // non-newline content, count it as one final line.
                    let scanned_end = file_offset + len as u64;
                    if scanned_end >= self.file_size && pos < chunk.len() {
                        line_count += 1;
                    }
                    break;
                }
            }
        }

        self.total_lines.store(line_count, Ordering::Relaxed);
        let scanned = file_offset + len as u64;
        self.bytes_scanned.store(scanned, Ordering::Relaxed);

        if self.file_size > 0 {
            let permille = ((scanned as f64 / self.file_size as f64) * 1000.0) as u32;
            self.progress.store(permille.min(1000), Ordering::Relaxed);
        }
    }

    /// Build the full index synchronously. Used for files that fit in the initial scan
    /// or as a fallback.
    pub fn build_full(&mut self, data: &[u8]) {
        if self.complete.load(Ordering::Acquire) {
            return;
        }
        self.sparse.clear();
        self.total_lines.store(0, Ordering::Relaxed);

        let scan_len = data.len();
        self.scan_chunk(data, 0, scan_len);

        // Handle last line (file may not end with newline)
        if scan_len > 0 && data[scan_len - 1] != b'\n' {
            let current = self.total_lines.load(Ordering::Relaxed);
            // Already counted in scan_chunk
            let _ = current;
        }

        self.complete.store(true, Ordering::Release);
        self.progress.store(1000, Ordering::Release);
    }

    /// Get the approximate total number of lines.
    /// If indexing is complete, this is exact.
    pub fn total_lines(&self) -> u64 {
        self.total_lines.load(Ordering::Relaxed)
    }

    /// Get the file offset for a given line number.
    /// Uses the sparse index to find the nearest group, then scans forward.
    /// Returns None if the line is beyond what's been indexed.
    pub fn line_offset(&self, data: &[u8], line: u64) -> Option<u64> {
        let total = self.total_lines.load(Ordering::Relaxed);
        if line > total {
            return None;
        }

        // Find the nearest sparse entry <= target group
        let target_group = line / GROUP_SIZE;
        let idx = self.sparse.partition_point(|(g, _)| *g <= target_group);
        let (start_group, start_offset) = if idx > 0 {
            self.sparse[idx - 1]
        } else {
            (0, 0)
        };

        // Count lines from start_offset to find the target line
        let start_line = start_group * GROUP_SIZE;
        if start_line == line {
            return Some(start_offset);
        }

        let mut current_line = start_line;
        let mut pos = start_offset as usize;

        while pos < data.len() && current_line < line {
            match memchr::memchr(b'\n', &data[pos..]) {
                Some(nl) => {
                    current_line += 1;
                    pos += nl + 1;
                    if current_line == line {
                        return Some(pos as u64);
                    }
                }
                None => break,
            }
        }

        // If we're at the end and this is the last line
        if current_line == line {
            Some(pos as u64)
        } else {
            None
        }
    }

    /// Extract `count` lines starting from `start_line`.
    /// Returns the lines as strings and the actual start line.
    pub fn get_lines(&self, data: &[u8], start_line: u64, count: u32) -> Vec<String> {
        let mut lines = Vec::with_capacity(count as usize);

        let start_offset = match self.line_offset(data, start_line) {
            Some(off) => off,
            None => return lines,
        };

        let mut pos = start_offset as usize;
        let remaining = count;

        for _ in 0..remaining {
            if pos >= data.len() {
                break;
            }

            // Find next newline
            match memchr::memchr(b'\n', &data[pos..]) {
                Some(nl) => {
                    let line_end = pos + nl;
                    // Check for CRLF
                    let line_end = if line_end > pos && data[line_end - 1] == b'\r' {
                        line_end - 1
                    } else {
                        line_end
                    };
                    let line = String::from_utf8_lossy(&data[pos..line_end]).into_owned();
                    lines.push(line);
                    pos += nl + 1;
                }
                None => {
                    // Last line without newline
                    let line_end = data.len();
                    let line_end = if line_end > pos && data[line_end - 1] == b'\r' {
                        line_end - 1
                    } else {
                        line_end
                    };
                    let line = String::from_utf8_lossy(&data[pos..line_end]).into_owned();
                    lines.push(line);
                    break;
                }
            }
        }

        lines
    }

    /// Get indexing progress as 0.0..1.0.
    pub fn progress(&self) -> f32 {
        self.progress.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Whether full indexing is complete.
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// Get the file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_count_small() {
        let data = b"line1\nline2\nline3\n";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);
        assert_eq!(idx.total_lines(), 3);
    }

    #[test]
    fn test_line_count_no_trailing_newline() {
        let data = b"line1\nline2\nline3";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);
        assert_eq!(idx.total_lines(), 3);
    }

    #[test]
    fn test_line_count_empty() {
        let data = b"";
        let mut idx = LineIndex::new(data, 0);
        idx.build_full(data);
        assert_eq!(idx.total_lines(), 0);
    }

    #[test]
    fn test_line_count_single_line() {
        let data = b"hello world";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);
        assert_eq!(idx.total_lines(), 1);
    }

    #[test]
    fn test_get_lines_from_start() {
        let data = b"aaa\nbbb\nccc\nddd\neee\n";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);

        let lines = idx.get_lines(data, 0, 3);
        assert_eq!(lines, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn test_get_lines_from_middle() {
        let data = b"aaa\nbbb\nccc\nddd\neee\n";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);

        let lines = idx.get_lines(data, 2, 2);
        assert_eq!(lines, vec!["ccc", "ddd"]);
    }

    #[test]
    fn test_get_lines_beyond_end() {
        let data = b"aaa\nbbb\nccc\n";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);

        let lines = idx.get_lines(data, 1, 100);
        assert_eq!(lines, vec!["bbb", "ccc"]);
    }

    #[test]
    fn test_sparse_index_lookup() {
        // Build data with enough lines to create multiple sparse groups
        let mut data = String::new();
        for i in 0..5000u64 {
            data.push_str(&format!("Line number {}\n", i));
        }
        let bytes = data.as_bytes();
        let mut idx = LineIndex::new(bytes, bytes.len() as u64);
        idx.build_full(bytes);

        assert_eq!(idx.total_lines(), 5000);
        assert!(idx.is_complete());

        // Verify sparse index was built (should have ~5 groups: 0, 1024, 2048, 3072, 4096)
        assert!(idx.sparse.len() >= 4, "Expected at least 4 sparse entries, got {}", idx.sparse.len());

        // Verify lookup at sparse group boundary
        let lines = idx.get_lines(bytes, 0, 1);
        assert_eq!(lines[0], "Line number 0");

        let lines = idx.get_lines(bytes, 1024, 1);
        assert_eq!(lines[0], "Line number 1024");

        let lines = idx.get_lines(bytes, 4096, 1);
        assert_eq!(lines[0], "Line number 4096");

        // Verify lookup at arbitrary line
        let lines = idx.get_lines(bytes, 2500, 1);
        assert_eq!(lines[0], "Line number 2500");

        // Verify lookup at last line
        let lines = idx.get_lines(bytes, 4999, 1);
        assert_eq!(lines[0], "Line number 4999");
    }

    #[test]
    fn test_crlf_handling() {
        let data = b"line1\r\nline2\r\nline3\r\n";
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);

        let lines = idx.get_lines(data, 0, 3);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_large_file_simulation() {
        // Simulate a 1MB file with ~10k lines
        let mut data = String::new();
        for i in 0..10000u64 {
            data.push_str(&format!("Line {}: This is test content with some padding data here. {}\n", i, "x".repeat(50)));
        }
        let bytes = data.as_bytes();
        assert!(bytes.len() > 1_000_000, "Test file should be > 1MB, got {} bytes", bytes.len());

        let mut idx = LineIndex::new(bytes, bytes.len() as u64);
        idx.build_full(bytes);

        assert_eq!(idx.total_lines(), 10000);

        // Test random access
        let lines = idx.get_lines(bytes, 0, 1);
        assert!(lines[0].starts_with("Line 0:"));

        let lines = idx.get_lines(bytes, 5000, 1);
        assert!(lines[0].starts_with("Line 5000:"));

        let lines = idx.get_lines(bytes, 9999, 1);
        assert!(lines[0].starts_with("Line 9999:"));

        // Test viewport-style request
        let lines = idx.get_lines(bytes, 100, 50);
        assert_eq!(lines.len(), 50);
        assert!(lines[0].starts_with("Line 100:"));
        assert!(lines[49].starts_with("Line 149:"));
    }
}
