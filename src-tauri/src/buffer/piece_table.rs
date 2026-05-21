use std::collections::BTreeMap;

/// Identifies which buffer a piece references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum PieceSource {
    /// The original file content (read-only, UTF-8 converted).
    Original,
    /// The append buffer for user edits.
    AddBuffer,
}

/// A contiguous range of text from a specific source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Piece {
    pub source: PieceSource,
    /// Byte offset within the source buffer.
    pub offset: u64,
    /// Byte length of this piece.
    pub length: u64,
}

/// A record for undo/redo.
#[derive(Debug, Clone)]
pub enum EditAction {
    Insert { pos: u64, text: Vec<u8> },
    Delete { pos: u64, length: u64, text: Vec<u8> },
    Replace { pos: u64, old_text: Vec<u8>, new_text: Vec<u8> },
}

/// Piece Table: an efficient data structure for text editing.
///
/// Stores the original text as read-only, and all edits go to an append-only
/// add buffer. A sorted list of "pieces" describes the logical document order.
pub struct PieceTable {
    /// Original file content (UTF-8).
    original: Vec<u8>,
    /// Append buffer for edits.
    add_buffer: Vec<u8>,
    /// Pieces describing the logical document, keyed by logical start offset.
    pieces: BTreeMap<u64, Piece>,
    /// Total logical length in bytes.
    total_len: u64,
    /// Undo stack.
    undo_stack: Vec<EditAction>,
    /// Redo stack.
    redo_stack: Vec<EditAction>,
}

impl PieceTable {
    /// Create a new PieceTable with the given original content (UTF-8 bytes).
    pub fn new(original: Vec<u8>) -> Self {
        let total_len = original.len() as u64;
        let mut pieces = BTreeMap::new();
        if total_len > 0 {
            pieces.insert(
                0,
                Piece {
                    source: PieceSource::Original,
                    offset: 0,
                    length: total_len,
                },
            );
        }
        Self {
            original,
            add_buffer: Vec::new(),
            pieces,
            total_len,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Total logical length in bytes.
    pub fn len(&self) -> u64 {
        self.total_len
    }

    /// Whether the document is empty.
    pub fn is_empty(&self) -> bool {
        self.total_len == 0
    }

    /// Whether there are any edits (non-original pieces).
    pub fn is_dirty(&self) -> bool {
        self.undo_stack.len() > 0
    }

    /// Number of pieces (for diagnostics).
    pub fn piece_count(&self) -> usize {
        self.pieces.len()
    }

    // ─── Insert ───────────────────────────────────────────────

    /// Insert text at the given byte position.
    pub fn insert(&mut self, pos: u64, text: &[u8]) {
        if text.is_empty() {
            return;
        }
        let pos = pos.min(self.total_len);
        let length = text.len() as u64;

        // Append to add buffer
        let add_offset = self.add_buffer.len() as u64;
        self.add_buffer.extend_from_slice(text);

        let new_piece = Piece {
            source: PieceSource::AddBuffer,
            offset: add_offset,
            length,
        };

        if self.pieces.is_empty() {
            // Document was empty
            self.pieces.insert(0, new_piece);
        } else {
            self.insert_piece_at(pos, new_piece);
        }

        self.total_len += length;

        // Record undo
        self.undo_stack.push(EditAction::Insert { pos, text: text.to_vec() });
        self.redo_stack.clear();
    }

    // ─── Delete ───────────────────────────────────────────────

    /// Delete `len` bytes starting at byte position `pos`.
    pub fn delete(&mut self, pos: u64, len: u64) {
        if len == 0 || pos >= self.total_len {
            return;
        }
        let len = len.min(self.total_len - pos);

        // Save deleted text for undo
        let deleted_text = self.read_range(pos, len);

        self.delete_range_internal(pos, len);
        self.total_len -= len;

        self.undo_stack.push(EditAction::Delete {
            pos,
            length: len,
            text: deleted_text,
        });
        self.redo_stack.clear();
    }

    // ─── Replace ──────────────────────────────────────────────

    /// Replace `len` bytes at `pos` with `text`.
    pub fn replace(&mut self, pos: u64, len: u64, text: &[u8]) {
        if pos >= self.total_len {
            // Append instead
            self.insert(self.total_len, text);
            return;
        }
        let len = len.min(self.total_len - pos);
        let old_text = self.read_range(pos, len);

        // Delete old content
        self.delete_range_internal(pos, len);
        self.total_len -= len;

        // Insert new content
        if !text.is_empty() {
            let add_offset = self.add_buffer.len() as u64;
            self.add_buffer.extend_from_slice(text);
            let new_len = text.len() as u64;

            let new_piece = Piece {
                source: PieceSource::AddBuffer,
                offset: add_offset,
                length: new_len,
            };
            self.insert_piece_at(pos, new_piece);
            self.total_len += new_len;
        }

        self.undo_stack.push(EditAction::Replace {
            pos,
            old_text,
            new_text: text.to_vec(),
        });
        self.redo_stack.clear();
    }

    // ─── Read ─────────────────────────────────────────────────

    /// Read `len` bytes starting at logical position `pos`.
    pub fn read_range(&self, pos: u64, len: u64) -> Vec<u8> {
        if len == 0 || pos >= self.total_len {
            return Vec::new();
        }
        let len = len.min(self.total_len - pos);
        let mut result = Vec::with_capacity(len as usize);
        let end = pos + len;

        // Find the first piece that overlaps with [pos, end)
        // We need to search for pieces where piece_start < end and piece_end > pos
        for (_, piece) in self.pieces.range(..end) {
            let piece_start = self.piece_logical_start(piece);
            let piece_end = piece_start + piece.length;
            if piece_end <= pos {
                continue;
            }

            let read_start = pos.max(piece_start) - piece_start;
            let read_end = (end.min(piece_end)) - piece_start;
            let read_len = read_end - read_start;

            let source = match piece.source {
                PieceSource::Original => &self.original,
                PieceSource::AddBuffer => &self.add_buffer,
            };

            let s = (piece.offset + read_start) as usize;
            let e = s + read_len as usize;
            result.extend_from_slice(&source[s..e]);

            if result.len() as u64 >= len {
                break;
            }
        }

        result
    }

    /// Read the entire logical content as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        self.read_range(0, self.total_len)
    }

    /// Read the entire logical content as a UTF-8 string (lossy).
    pub fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(&self.to_bytes()).into_owned()
    }

    // ─── Undo / Redo ─────────────────────────────────────────

    /// Undo the last edit action. Returns true if something was undone.
    pub fn undo(&mut self) -> bool {
        let action = match self.undo_stack.pop() {
            Some(a) => a,
            None => return false,
        };

        match &action {
            EditAction::Insert { pos, text } => {
                let len = text.len() as u64;
                self.delete_range_internal(*pos, len);
                self.total_len -= len;
            }
            EditAction::Delete { pos, length: _, text } => {
                // Re-insert the deleted text
                self.insert_raw(*pos, text);
                self.total_len += text.len() as u64;
            }
            EditAction::Replace { pos, old_text, new_text } => {
                let new_len = new_text.len() as u64;
                self.delete_range_internal(*pos, new_len);
                self.total_len -= new_len;
                if !old_text.is_empty() {
                    self.insert_raw(*pos, old_text);
                    self.total_len += old_text.len() as u64;
                }
            }
        }

        self.redo_stack.push(action);
        true
    }

    /// Redo the last undone action. Returns true if something was redone.
    pub fn redo(&mut self) -> bool {
        let action = match self.redo_stack.pop() {
            Some(a) => a,
            None => return false,
        };

        match &action {
            EditAction::Insert { pos, text } => {
                self.insert_raw(*pos, text);
                self.total_len += text.len() as u64;
            }
            EditAction::Delete { pos, length, text: _ } => {
                self.delete_range_internal(*pos, *length);
                self.total_len -= length;
            }
            EditAction::Replace { pos, old_text, new_text } => {
                if !old_text.is_empty() {
                    self.delete_range_internal(*pos, old_text.len() as u64);
                    self.total_len -= old_text.len() as u64;
                }
                if !new_text.is_empty() {
                    self.insert_raw(*pos, new_text);
                    self.total_len += new_text.len() as u64;
                }
            }
        }

        self.undo_stack.push(action);
        true
    }

    // ─── Internal helpers ─────────────────────────────────────

    /// Get the logical start offset of a piece.
    /// This requires knowing which key the piece is stored under.
    /// Since we use BTreeMap keyed by logical offset, we need to find it.
    fn piece_logical_start(&self, target: &Piece) -> u64 {
        // Find the key for this piece in the BTreeMap
        for (key, piece) in &self.pieces {
            if std::ptr::eq(piece as *const Piece, target as *const Piece) {
                return *key;
            }
        }
        // Fallback: scan
        let mut offset = 0u64;
        for (_, piece) in &self.pieces {
            if piece.source == target.source
                && piece.offset == target.offset
                && piece.length == target.length
            {
                return offset;
            }
            offset += piece.length;
        }
        0
    }

    /// Insert a piece at logical position `pos`, splitting existing pieces as needed.
    fn insert_piece_at(&mut self, pos: u64, new_piece: Piece) {
        let new_len = new_piece.length;

        // Collect all pieces, split at `pos`, insert new_piece, rebuild
        let ordered: Vec<(u64, Piece)> = self.pieces.iter().map(|(&k, p)| (k, p.clone())).collect();
        self.pieces.clear();

        let mut result: Vec<(u64, Piece)> = Vec::new();
        let mut inserted = false;
        let mut current_offset = 0u64;

        for (_, piece) in &ordered {
            let piece_start = current_offset;
            let piece_end = piece_start + piece.length;

            if !inserted && pos <= piece_start {
                // Insert before this piece
                result.push((pos, new_piece.clone()));
                inserted = true;
            }

            if !inserted && pos > piece_start && pos < piece_end {
                // Split this piece at `pos`
                let split_at = pos - piece_start;

                // Left part
                result.push((
                    piece_start,
                    Piece {
                        source: piece.source,
                        offset: piece.offset,
                        length: split_at,
                    },
                ));

                // New piece
                result.push((pos, new_piece.clone()));
                inserted = true;

                // Right part
                result.push((
                    pos + new_len,
                    Piece {
                        source: piece.source,
                        offset: piece.offset + split_at,
                        length: piece.length - split_at,
                    },
                ));
            } else if inserted {
                // Shift this piece by new_len
                result.push((piece_start + new_len, piece.clone()));
            } else {
                result.push((piece_start, piece.clone()));
            }

            current_offset = piece_end;
        }

        if !inserted {
            result.push((pos, new_piece));
        }

        // Rebuild BTreeMap
        for (key, piece) in result {
            if piece.length > 0 {
                self.pieces.insert(key, piece);
            }
        }
    }

    /// Delete `len` bytes at logical position `pos` (internal, no undo recording).
    fn delete_range_internal(&mut self, pos: u64, len: u64) {
        if len == 0 || pos >= self.total_len {
            return;
        }
        let end = (pos + len).min(self.total_len);

        let ordered: Vec<(u64, Piece)> = self.pieces.iter().map(|(&k, p)| (k, p.clone())).collect();
        self.pieces.clear();

        let mut result: Vec<(u64, Piece)> = Vec::new();
        let mut current_offset = 0u64;

        for (_, piece) in &ordered {
            let piece_start = current_offset;
            let piece_end = piece_start + piece.length;

            if piece_end <= pos || piece_start >= end {
                // No overlap with deletion range
                let shift = if piece_start >= end { len } else { 0 };
                result.push((piece_start - shift, piece.clone()));
            } else if piece_start >= pos && piece_end <= end {
                // Entirely within deletion range — skip
            } else if piece_start < pos && piece_end > end {
                // Deletion range is in the middle — split into two
                let left_len = pos - piece_start;
                let right_start_in_piece = end - piece_start;
                let right_len = piece.length - right_start_in_piece;

                result.push((
                    piece_start,
                    Piece {
                        source: piece.source,
                        offset: piece.offset,
                        length: left_len,
                    },
                ));
                result.push((
                    pos, // right part starts at pos after deletion
                    Piece {
                        source: piece.source,
                        offset: piece.offset + right_start_in_piece,
                        length: right_len,
                    },
                ));
            } else if piece_start < pos {
                // Partial overlap from left — keep left part
                let keep_len = pos - piece_start;
                result.push((
                    piece_start,
                    Piece {
                        source: piece.source,
                        offset: piece.offset,
                        length: keep_len,
                    },
                ));
            } else {
                // Partial overlap from right — keep right part
                let skip_len = end - piece_start;
                let keep_offset = piece.offset + skip_len;
                let keep_len = piece.length - skip_len;
                result.push((
                    pos, // after deletion, starts at pos
                    Piece {
                        source: piece.source,
                        offset: keep_offset,
                        length: keep_len,
                    },
                ));
            }

            current_offset = piece_end;
        }

        for (key, piece) in result {
            if piece.length > 0 {
                self.pieces.insert(key, piece);
            }
        }
    }

    /// Insert raw text at position (for undo), reusing add_buffer.
    fn insert_raw(&mut self, pos: u64, text: &[u8]) {
        if text.is_empty() {
            return;
        }
        let add_offset = self.add_buffer.len() as u64;
        self.add_buffer.extend_from_slice(text);

        let new_piece = Piece {
            source: PieceSource::AddBuffer,
            offset: add_offset,
            length: text.len() as u64,
        };

        if self.pieces.is_empty() {
            self.pieces.insert(0, new_piece);
        } else {
            self.insert_piece_at(pos, new_piece);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(text: &str) -> PieceTable {
        PieceTable::new(text.as_bytes().to_vec())
    }

    #[test]
    fn test_empty() {
        let p = pt("");
        assert!(p.is_empty());
        assert_eq!(p.len(), 0);
        assert_eq!(p.to_bytes(), b"");
    }

    #[test]
    fn test_original_only() {
        let p = pt("Hello, World!");
        assert_eq!(p.len(), 13);
        assert_eq!(p.to_bytes(), b"Hello, World!");
        assert!(!p.is_dirty());
    }

    #[test]
    fn test_insert_at_start() {
        let mut p = pt("World");
        p.insert(0, b"Hello, ");
        assert_eq!(p.to_string_lossy(), "Hello, World");
        assert_eq!(p.len(), 12);
    }

    #[test]
    fn test_insert_at_end() {
        let mut p = pt("Hello");
        p.insert(5, b", World!");
        assert_eq!(p.to_string_lossy(), "Hello, World!");
    }

    #[test]
    fn test_insert_in_middle() {
        let mut p = pt("Helo");
        p.insert(2, b"l");
        assert_eq!(p.to_string_lossy(), "Hello");
    }

    #[test]
    fn test_insert_into_empty() {
        let mut p = pt("");
        p.insert(0, b"Hello");
        assert_eq!(p.to_string_lossy(), "Hello");
        assert_eq!(p.len(), 5);
    }

    #[test]
    fn test_delete_from_start() {
        let mut p = pt("Hello, World!");
        p.delete(0, 7);
        assert_eq!(p.to_string_lossy(), "World!");
    }

    #[test]
    fn test_delete_from_end() {
        let mut p = pt("Hello, World!");
        p.delete(5, 8);
        assert_eq!(p.to_string_lossy(), "Hello");
    }

    #[test]
    fn test_delete_from_middle() {
        let mut p = pt("Hello, World!");
        p.delete(5, 2);
        assert_eq!(p.to_string_lossy(), "HelloWorld!");
    }

    #[test]
    fn test_delete_all() {
        let mut p = pt("Hello");
        p.delete(0, 5);
        assert!(p.is_empty());
    }

    #[test]
    fn test_replace_basic() {
        let mut p = pt("Hello, World!");
        p.replace(7, 5, b"Rust");
        assert_eq!(p.to_string_lossy(), "Hello, Rust!");
    }

    #[test]
    fn test_replace_different_length() {
        let mut p = pt("abc");
        p.replace(1, 1, b"XYZ");
        assert_eq!(p.to_string_lossy(), "aXYZc");
    }

    #[test]
    fn test_replace_entire_content() {
        let mut p = pt("old content");
        p.replace(0, 11, b"new content");
        assert_eq!(p.to_string_lossy(), "new content");
    }

    #[test]
    fn test_multiple_inserts() {
        let mut p = pt("ac");
        p.insert(1, b"b");
        assert_eq!(p.to_string_lossy(), "abc");
        p.insert(3, b"d");
        assert_eq!(p.to_string_lossy(), "abcd");
    }

    #[test]
    fn test_interleaved_edits() {
        let mut p = pt("The quick fox");
        // Insert "brown " before "fox"
        p.insert(10, b"brown ");
        assert_eq!(p.to_string_lossy(), "The quick brown fox");
        // Insert "lazy " before "fox"
        p.insert(16, b"lazy ");
        assert_eq!(p.to_string_lossy(), "The quick brown lazy fox");
    }

    #[test]
    fn test_read_range() {
        let mut p = pt("Hello, World!");
        p.insert(5, b" Beautiful");
        // "Hello Beautiful, World!"
        assert_eq!(p.read_range(0, 5), b"Hello");
        assert_eq!(p.read_range(5, 10), b" Beautiful");
        assert_eq!(p.read_range(15, 7), b", World");
    }

    #[test]
    fn test_undo_insert() {
        let mut p = pt("Hello");
        p.insert(5, b" World");
        assert_eq!(p.to_string_lossy(), "Hello World");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello");
    }

    #[test]
    fn test_undo_delete() {
        let mut p = pt("Hello, World!");
        p.delete(5, 7);
        assert_eq!(p.to_string_lossy(), "Hello!");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello, World!");
    }

    #[test]
    fn test_undo_replace() {
        let mut p = pt("Hello, World!");
        p.replace(7, 5, b"Rust");
        assert_eq!(p.to_string_lossy(), "Hello, Rust!");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello, World!");
    }

    #[test]
    fn test_undo_multiple() {
        let mut p = pt("a");
        p.insert(1, b"b");
        p.insert(2, b"c");
        assert_eq!(p.to_string_lossy(), "abc");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "ab");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "a");
    }

    #[test]
    fn test_undo_empty() {
        let mut p = pt("Hello");
        assert!(!p.undo());
        assert_eq!(p.to_string_lossy(), "Hello");
    }

    #[test]
    fn test_redo_insert() {
        let mut p = pt("Hello");
        p.insert(5, b" World");
        assert_eq!(p.to_string_lossy(), "Hello World");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello");
        assert!(p.redo());
        assert_eq!(p.to_string_lossy(), "Hello World");
    }

    #[test]
    fn test_redo_delete() {
        let mut p = pt("Hello, World!");
        p.delete(5, 7);
        assert_eq!(p.to_string_lossy(), "Hello!");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello, World!");
        assert!(p.redo());
        assert_eq!(p.to_string_lossy(), "Hello!");
    }

    #[test]
    fn test_redo_replace() {
        let mut p = pt("Hello, World!");
        p.replace(7, 5, b"Rust");
        assert_eq!(p.to_string_lossy(), "Hello, Rust!");
        assert!(p.undo());
        assert_eq!(p.to_string_lossy(), "Hello, World!");
        assert!(p.redo());
        assert_eq!(p.to_string_lossy(), "Hello, Rust!");
    }

    #[test]
    fn test_redo_empty() {
        let mut p = pt("Hello");
        assert!(!p.redo());
    }

    #[test]
    fn test_undo_redo_cycle() {
        let mut p = pt("a");
        p.insert(1, b"b");  // "ab"
        p.insert(2, b"c");  // "abc"
        p.delete(0, 1);     // "bc"
        assert_eq!(p.to_string_lossy(), "bc");

        assert!(p.undo());  // undo delete -> "abc"
        assert_eq!(p.to_string_lossy(), "abc");
        assert!(p.undo());  // undo insert c -> "ab"
        assert_eq!(p.to_string_lossy(), "ab");
        assert!(p.redo());  // redo insert c -> "abc"
        assert_eq!(p.to_string_lossy(), "abc");
        assert!(p.redo());  // redo delete -> "bc"
        assert_eq!(p.to_string_lossy(), "bc");
    }

    #[test]
    fn test_new_edit_clears_redo() {
        let mut p = pt("Hello");
        p.insert(5, b" World");
        assert!(p.undo());
        // Now redo stack has an entry
        p.insert(5, b" Rust"); // New edit should clear redo
        assert!(!p.redo()); // Nothing to redo
        assert_eq!(p.to_string_lossy(), "Hello Rust");
    }

    #[test]
    fn test_piece_count() {
        let mut p = pt("Hello");
        assert_eq!(p.piece_count(), 1);
        p.insert(5, b" World");
        assert_eq!(p.piece_count(), 2);
        p.insert(5, b" Beautiful");
        assert_eq!(p.piece_count(), 3);
    }

    #[test]
    fn test_large_file_simulation() {
        // Simulate many edits on a large-ish document
        let original = "a".repeat(100_000);
        let mut p = pt(&original);
        assert_eq!(p.len(), 100_000);

        // Insert at various positions
        for i in 0..100 {
            let pos = (i * 1000) + i;
            p.insert(pos as u64, b"INSERTED");
        }
        assert_eq!(p.len(), 100_800);

        // Delete some
        for i in 0..50 {
            let pos = (i * 2000) + 100;
            p.delete(pos as u64, 10);
        }
        assert_eq!(p.len(), 100_300);

        // Verify we can still read
        let bytes = p.read_range(0, 100);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_stress_insert_delete_cycle() {
        let mut p = pt("");
        for i in 0..200 {
            p.insert(p.len(), format!("line {}\n", i).as_bytes());
        }
        let content = p.to_string_lossy();
        assert!(content.contains("line 0\n"));
        assert!(content.contains("line 199\n"));

        // Delete every other line
        for i in (0..200).rev().step_by(2) {
            let line_start = content.lines().take(i).map(|l| l.len() + 1).sum::<usize>();
            let line_len = format!("line {}\n", i).len();
            p.delete(line_start as u64, line_len as u64);
        }
    }
}
