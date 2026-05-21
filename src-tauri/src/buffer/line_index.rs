use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use lru::LruCache;
use parking_lot::Mutex;

/// Number of lines per main sparse index group.
/// Every GROUP_SIZE lines, we record the file offset.
/// For 100GB (~10B lines): 10B / 4096 ≈ 2.4M entries ≈ 19MB.
const GROUP_SIZE: u64 = 4096;

/// Maximum bytes to scan synchronously on file open.
const INITIAL_SCAN_BYTES: usize = 2 * 1024 * 1024; // 2MB

/// Size of chunks processed during background indexing.
const CHUNK_SIZE: usize = 4 * 1024 * 1024; // 4MB

/// Block-level cache entry: line offsets within a group.
struct BlockIndex {
    /// Byte offset of each line within this group.
    /// lines[0] is the offset of the first line in the group.
    lines: Vec<u64>,
}

/// A two-level sparse line index with background building support.
///
/// Level 1 (main): Sparse entries every GROUP_SIZE lines. Always in memory.
/// Level 2 (block): Per-group line offsets. LRU-cached on demand.
pub struct LineIndex {
    /// Main sparse index: group_number -> byte offset of first line in group.
    sparse: parking_lot::RwLock<Vec<(u64, u64)>>,

    /// Block cache: recently accessed groups' per-line offsets.
    block_cache: Mutex<LruCache<u64, BlockIndex>>,

    /// Total number of lines discovered.
    total_lines: AtomicU64,

    /// Total file size.
    file_size: u64,

    /// Indexing progress per-mille (0..1000).
    progress: AtomicU32,

    /// Whether full indexing is complete.
    complete: AtomicBool,
}

// BlockIndex is Send+Sync (it's just a Vec<u64>)
unsafe impl Send for LineIndex {}
unsafe impl Sync for LineIndex {}

impl LineIndex {
    /// Create a new LineIndex by scanning the first ~2MB of data.
    /// Returns immediately for fast initial display.
    pub fn new(data: &[u8], file_size: u64) -> Self {
        let idx = Self {
            sparse: parking_lot::RwLock::new(Vec::new()),
            block_cache: Mutex::new(LruCache::new(std::num::NonZeroUsize::new(64).unwrap())),
            total_lines: AtomicU64::new(0),
            file_size,
            progress: AtomicU32::new(0),
            complete: AtomicBool::new(false),
        };

        let scan_len = data.len().min(INITIAL_SCAN_BYTES);
        idx.scan_range(data, 0, scan_len);

        if scan_len >= data.len() {
            idx.complete.store(true, Ordering::Release);
            idx.progress.store(1000, Ordering::Release);
        }

        idx
    }

    /// Build the full index synchronously. For small files or fallback.
    pub fn build_full(&mut self, data: &[u8]) {
        if self.complete.load(Ordering::Acquire) {
            return;
        }
        {
            let mut sparse = self.sparse.write();
            sparse.clear();
        }
        self.total_lines.store(0, Ordering::Relaxed);

        self.scan_range(data, 0, data.len());
        self.complete.store(true, Ordering::Release);
        self.progress.store(1000, Ordering::Release);
    }

    /// Build the index in a background thread. Non-blocking.
    /// `data` must be the full file content (Arc for shared ownership).
    /// `on_progress` is called periodically with (lines, progress_permille).
    pub fn build_background(
        self: &Arc<Self>,
        data: Arc<Vec<u8>>,
        on_progress: Option<Arc<dyn Fn(u64, u32) + Send + Sync>>,
    ) {
        let this = Arc::clone(self);

        // Already complete?
        if this.complete.load(Ordering::Acquire) {
            return;
        }

        std::thread::spawn(move || {
            let data_len = data.len();
            let mut offset = INITIAL_SCAN_BYTES.min(data_len);

            while offset < data_len {
                let chunk_len = CHUNK_SIZE.min(data_len - offset);
                this.scan_range(&data, offset, chunk_len);
                offset += chunk_len;

                if let Some(ref cb) = on_progress {
                    let lines = this.total_lines.load(Ordering::Relaxed);
                    let prog = this.progress.load(Ordering::Relaxed);
                    cb(lines, prog);
                }

                // Yield to avoid starving other threads
                std::thread::yield_now();
            }

            this.complete.store(true, Ordering::Release);
            this.progress.store(1000, Ordering::Release);

            if let Some(ref cb) = on_progress {
                let lines = this.total_lines.load(Ordering::Relaxed);
                cb(lines, 1000);
            }
        });
    }

    // ─── Internal scanning ────────────────────────────────────

    /// Scan a byte range and update the sparse index.
    fn scan_range(&self, data: &[u8], file_offset: usize, len: usize) {
        if len == 0 || file_offset >= data.len() {
            return;
        }
        let end = (file_offset + len).min(data.len());
        let chunk = &data[file_offset..end];

        let mut line_count = self.total_lines.load(Ordering::Relaxed);
        let mut pos: usize = 0;
        let base_offset = file_offset as u64;

        let mut sparse = self.sparse.write();

        // Record first line of this chunk if it starts a new group
        let group = line_count / GROUP_SIZE;
        if sparse.last().map_or(true, |(g, _)| *g != group) {
            sparse.push((group, base_offset));
        }

        while pos < chunk.len() {
            match memchr::memchr(b'\n', &chunk[pos..]) {
                Some(nl_pos) => {
                    line_count += 1;
                    pos += nl_pos + 1;

                    let group = line_count / GROUP_SIZE;
                    if sparse.last().map_or(true, |(g, _)| *g != group) {
                        sparse.push((group, base_offset + pos as u64));
                    }
                }
                None => {
                    // Count trailing content as a line
                    let scanned_end = file_offset + len;
                    if scanned_end >= data.len() && pos < chunk.len() {
                        line_count += 1;
                    }
                    break;
                }
            }
        }

        drop(sparse);

        self.total_lines.store(line_count, Ordering::Relaxed);

        if self.file_size > 0 {
            let scanned = (file_offset + len) as u64;
            let permille = ((scanned as f64 / self.file_size as f64) * 1000.0) as u32;
            self.progress.store(permille.min(1000), Ordering::Relaxed);
        }
    }

    // ─── Lookup ───────────────────────────────────────────────

    /// Get the byte offset for a given line number.
    /// Uses the main sparse index, then the block cache, then scans.
    pub fn line_offset(&self, data: &[u8], line: u64) -> Option<u64> {
        let total = self.total_lines.load(Ordering::Relaxed);
        if line > total {
            return None;
        }

        let target_group = line / GROUP_SIZE;

        // Find nearest sparse entry <= target group
        let (start_group, start_offset) = {
            let sparse = self.sparse.read();
            let idx = sparse.partition_point(|(g, _)| *g <= target_group);
            if idx > 0 {
                sparse[idx - 1]
            } else {
                (0, 0)
            }
        };

        let start_line = start_group * GROUP_SIZE;
        if start_line == line {
            return Some(start_offset);
        }

        // Try block cache first
        {
            let mut cache = self.block_cache.lock();
            if let Some(block) = cache.get(&start_group) {
                let local_idx = (line - start_line) as usize;
                if local_idx < block.lines.len() {
                    return Some(block.lines[local_idx]);
                }
            }
        }

        // Build block index by scanning
        self.build_block_and_lookup(data, start_group, start_line, start_offset, line)
    }

    /// Build the block index for a group and return the offset for `target_line`.
    fn build_block_and_lookup(
        &self,
        data: &[u8],
        group: u64,
        start_line: u64,
        start_offset: u64,
        target_line: u64,
    ) -> Option<u64> {
        let mut lines = Vec::with_capacity(GROUP_SIZE as usize + 1);
        lines.push(start_offset);

        let mut pos = start_offset as usize;
        let mut current_line = start_line;
        let mut result = None;

        while pos < data.len() && current_line < start_line + GROUP_SIZE {
            match memchr::memchr(b'\n', &data[pos..]) {
                Some(nl) => {
                    current_line += 1;
                    pos += nl + 1;
                    lines.push(pos as u64);

                    if current_line == target_line {
                        result = Some(pos as u64);
                    }
                }
                None => {
                    // Last line without trailing newline
                    current_line += 1;
                    if current_line == target_line {
                        result = Some(pos as u64);
                    }
                    break;
                }
            }
        }

        // Cache the block
        let block = BlockIndex { lines };
        let mut cache = self.block_cache.lock();
        cache.put(group, block);

        // If target is beyond what we scanned
        if result.is_none() && target_line == start_line {
            result = Some(start_offset);
        }

        result
    }

    /// Extract `count` lines starting from `start_line`.
    pub fn get_lines(&self, data: &[u8], start_line: u64, count: u32) -> Vec<String> {
        let mut result = Vec::with_capacity(count as usize);

        let start_offset = match self.line_offset(data, start_line) {
            Some(off) => off,
            None => return result,
        };

        let mut pos = start_offset as usize;

        for _ in 0..count {
            if pos >= data.len() {
                break;
            }

            match memchr::memchr(b'\n', &data[pos..]) {
                Some(nl) => {
                    let line_end = if pos + nl > pos && data[pos + nl - 1] == b'\r' {
                        pos + nl - 1
                    } else {
                        pos + nl
                    };
                    let line = String::from_utf8_lossy(&data[pos..line_end]).into_owned();
                    result.push(line);
                    pos += nl + 1;
                }
                None => {
                    let line_end = data.len();
                    let line_end = if line_end > pos && data[line_end - 1] == b'\r' {
                        line_end - 1
                    } else {
                        line_end
                    };
                    let line = String::from_utf8_lossy(&data[pos..line_end]).into_owned();
                    result.push(line);
                    break;
                }
            }
        }

        result
    }

    /// Get approximate total lines.
    pub fn total_lines(&self) -> u64 {
        self.total_lines.load(Ordering::Relaxed)
    }

    /// Get indexing progress 0.0..1.0.
    pub fn progress(&self) -> f32 {
        self.progress.load(Ordering::Relaxed) as f32 / 1000.0
    }

    /// Whether indexing is complete.
    pub fn is_complete(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }

    /// Get file size.
    pub fn file_size(&self) -> u64 {
        self.file_size
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_index(data: &[u8]) -> LineIndex {
        let mut idx = LineIndex::new(data, data.len() as u64);
        idx.build_full(data);
        idx
    }

    #[test]
    fn test_line_count_small() {
        let data = b"line1\nline2\nline3\n";
        let idx = make_index(data);
        assert_eq!(idx.total_lines(), 3);
    }

    #[test]
    fn test_line_count_no_trailing_newline() {
        let data = b"line1\nline2\nline3";
        let idx = make_index(data);
        assert_eq!(idx.total_lines(), 3);
    }

    #[test]
    fn test_line_count_empty() {
        let data = b"";
        let idx = make_index(data);
        assert_eq!(idx.total_lines(), 0);
    }

    #[test]
    fn test_line_count_single_line() {
        let data = b"hello world";
        let idx = make_index(data);
        assert_eq!(idx.total_lines(), 1);
    }

    #[test]
    fn test_get_lines_from_start() {
        let data = b"aaa\nbbb\nccc\nddd\neee\n";
        let idx = make_index(data);
        let lines = idx.get_lines(data, 0, 3);
        assert_eq!(lines, vec!["aaa", "bbb", "ccc"]);
    }

    #[test]
    fn test_get_lines_from_middle() {
        let data = b"aaa\nbbb\nccc\nddd\neee\n";
        let idx = make_index(data);
        let lines = idx.get_lines(data, 2, 2);
        assert_eq!(lines, vec!["ccc", "ddd"]);
    }

    #[test]
    fn test_get_lines_beyond_end() {
        let data = b"aaa\nbbb\nccc\n";
        let idx = make_index(data);
        let lines = idx.get_lines(data, 1, 100);
        assert_eq!(lines, vec!["bbb", "ccc"]);
    }

    #[test]
    fn test_sparse_index_lookup() {
        let mut data = String::new();
        for i in 0..5000u64 {
            data.push_str(&format!("Line number {}\n", i));
        }
        let bytes = data.as_bytes();
        let idx = make_index(bytes);

        assert_eq!(idx.total_lines(), 5000);
        assert!(idx.is_complete());

        // Verify sparse index was built
        let sparse = idx.sparse.read();
        assert!(sparse.len() >= 2, "Expected sparse entries, got {}", sparse.len());
        drop(sparse);

        // Verify lookups
        let lines = idx.get_lines(bytes, 0, 1);
        assert_eq!(lines[0], "Line number 0");

        let lines = idx.get_lines(bytes, 1024, 1);
        assert_eq!(lines[0], "Line number 1024");

        let lines = idx.get_lines(bytes, 2500, 1);
        assert_eq!(lines[0], "Line number 2500");

        let lines = idx.get_lines(bytes, 4999, 1);
        assert_eq!(lines[0], "Line number 4999");
    }

    #[test]
    fn test_crlf_handling() {
        let data = b"line1\r\nline2\r\nline3\r\n";
        let idx = make_index(data);
        let lines = idx.get_lines(data, 0, 3);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_large_file_simulation() {
        let mut data = String::new();
        for i in 0..10000u64 {
            data.push_str(&format!("Line {}: This is test content with some padding data. {}\n", i, "x".repeat(80)));
        }
        let bytes = data.as_bytes();
        assert!(bytes.len() > 1_000_000, "Expected > 1MB, got {} bytes", bytes.len());

        let idx = make_index(bytes);
        assert_eq!(idx.total_lines(), 10000);

        let lines = idx.get_lines(bytes, 0, 1);
        assert!(lines[0].starts_with("Line 0:"));

        let lines = idx.get_lines(bytes, 5000, 1);
        assert!(lines[0].starts_with("Line 5000:"));

        let lines = idx.get_lines(bytes, 9999, 1);
        assert!(lines[0].starts_with("Line 9999:"));

        let lines = idx.get_lines(bytes, 100, 50);
        assert_eq!(lines.len(), 50);
    }

    #[test]
    fn test_block_cache() {
        let mut data = String::new();
        for i in 0..10000u64 {
            data.push_str(&format!("Line {}\n", i));
        }
        let bytes = data.as_bytes();
        let idx = make_index(bytes);

        // Access line within a group to trigger block cache
        let _ = idx.line_offset(bytes, 500);
        {
            let cache = idx.block_cache.lock();
            // Group 0 should be cached (500 / 4096 = 0)
            assert!(cache.peek(&0).is_some());
        }

        // Access line in a different group
        let _ = idx.line_offset(bytes, 5000);
        {
            let cache = idx.block_cache.lock();
            // Group 1 should be cached (5000 / 4096 = 1)
            assert!(cache.peek(&1).is_some());
        }
    }

    #[test]
    fn test_background_indexing() {
        let mut data = String::new();
        for i in 0..100000u64 {
            data.push_str(&format!("Line {}: some padding content to make lines longer {}\n", i, "x".repeat(80)));
        }
        let bytes = Arc::new(data.into_bytes());
        assert!(bytes.len() > 5_000_000, "Expected > 5MB, got {} bytes", bytes.len());

        let idx = Arc::new(LineIndex::new(&bytes, bytes.len() as u64));

        // Initially only ~2MB is indexed
        let initial_lines = idx.total_lines();
        assert!(initial_lines < 100000, "Expected partial index, got {}", initial_lines);

        // Build background
        let idx_clone = Arc::clone(&idx);
        let bytes_clone = Arc::clone(&bytes);
        idx_clone.build_background(bytes_clone, None);

        // Wait for completion (max 10s)
        for _ in 0..100 {
            if idx.is_complete() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        assert!(idx.is_complete(), "Background indexing did not complete");
        assert_eq!(idx.total_lines(), 100000);

        // Verify lookups work after background indexing
        let lines = idx.get_lines(&bytes, 0, 1);
        assert!(lines[0].starts_with("Line 0:"));

        let lines = idx.get_lines(&bytes, 99999, 1);
        assert!(lines[0].starts_with("Line 99999:"));
    }

    #[test]
    fn test_initial_scan_only() {
        // Create data larger than INITIAL_SCAN_BYTES (2MB)
        let mut data = String::new();
        for i in 0..100000u64 {
            data.push_str(&format!("Line {}: padding to fill up space {}\n", i, "x".repeat(80)));
        }
        let bytes = data.as_bytes();
        assert!(bytes.len() > 5_000_000, "Expected > 5MB, got {} bytes", bytes.len());

        let idx = LineIndex::new(bytes, bytes.len() as u64);

        // Should not be complete (file > 2MB)
        assert!(!idx.is_complete());
        assert!(idx.total_lines() < 100000, "Expected partial index, got {}", idx.total_lines());

        // But we should be able to access lines in the scanned region
        let lines = idx.get_lines(bytes, 0, 1);
        assert!(lines[0].starts_with("Line 0:"));
    }
}
