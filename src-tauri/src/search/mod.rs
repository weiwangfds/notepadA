pub mod text_search;
pub mod regex_search;

use serde::{Deserialize, Serialize};

/// Options for a search operation.
#[derive(Debug, Clone, Deserialize)]
pub struct SearchOptions {
    /// Whether the search is case-sensitive.
    pub case_sensitive: bool,
    /// Whether to match whole words only.
    pub whole_word: bool,
    /// Whether to use regex matching.
    pub regex: bool,
}

/// A single search match.
#[derive(Debug, Clone, Serialize)]
pub struct SearchMatch {
    /// 0-based line number.
    pub line: u64,
    /// 0-based column (byte offset within line).
    pub col: u32,
    /// Matched text length in bytes.
    pub length: u32,
    /// The matched text.
    pub text: String,
}
