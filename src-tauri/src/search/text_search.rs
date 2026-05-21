use super::{SearchMatch, SearchOptions};

/// Search for `query` in `content` using simple byte scanning.
/// Returns all matches with line/column information.
pub fn search(content: &[u8], query: &str, options: &SearchOptions) -> Vec<SearchMatch> {
    if query.is_empty() || content.is_empty() {
        return Vec::new();
    }

    let query_bytes = if options.case_sensitive {
        query.as_bytes().to_vec()
    } else {
        query.to_lowercase().into_bytes()
    };

    let mut matches = Vec::new();
    let mut line = 0u64;
    let mut line_start = 0usize;
    let mut pos = 0usize;

    while pos < content.len() {
        // Find the end of the current line
        let line_end = memchr::memchr(b'\n', &content[pos..])
            .map(|nl| pos + nl)
            .unwrap_or(content.len());

        let line_bytes = &content[line_start..line_end];

        // Search within this line
        let search_bytes = if options.case_sensitive {
            line_bytes.to_vec()
        } else {
            line_bytes.to_ascii_lowercase()
        };

        let mut col = 0;
        while col + query_bytes.len() <= search_bytes.len() {
            if &search_bytes[col..col + query_bytes.len()] == &query_bytes[..] {
                // Check whole word boundary if needed
                if options.whole_word {
                    let before_ok = col == 0 || !is_word_char(search_bytes[col - 1]);
                    let after_ok = col + query_bytes.len() >= search_bytes.len()
                        || !is_word_char(search_bytes[col + query_bytes.len()]);
                    if !before_ok || !after_ok {
                        col += 1;
                        continue;
                    }
                }

                let matched_text = String::from_utf8_lossy(
                    &line_bytes[col..col + query_bytes.len()],
                )
                .into_owned();

                matches.push(SearchMatch {
                    line,
                    col: col as u32,
                    length: query_bytes.len() as u32,
                    text: matched_text,
                });
            }
            col += 1;
        }

        // Move to next line
        pos = if line_end < content.len() {
            line_end + 1
        } else {
            content.len()
        };
        line_start = pos;
        line += 1;
    }

    matches
}

fn is_word_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(case_sensitive: bool, whole_word: bool) -> SearchOptions {
        SearchOptions {
            case_sensitive,
            whole_word,
            regex: false,
        }
    }

    #[test]
    fn test_basic_search() {
        let content = b"hello world\nhello rust\nfoo bar";
        let results = search(content, "hello", &opts(true, false));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].line, 0);
        assert_eq!(results[0].col, 0);
        assert_eq!(results[1].line, 1);
        assert_eq!(results[1].col, 0);
    }

    #[test]
    fn test_case_insensitive() {
        let content = b"Hello World\nHELLO Rust\nfoo bar";
        let results = search(content, "hello", &opts(false, false));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_case_sensitive() {
        let content = b"Hello World\nhello Rust";
        let results = search(content, "hello", &opts(true, false));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line, 1);
    }

    #[test]
    fn test_whole_word() {
        let content = b"hello world\nhelloworld foo";
        let results = search(content, "hello", &opts(true, true));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line, 0);
    }

    #[test]
    fn test_no_match() {
        let content = b"hello world";
        let results = search(content, "xyz", &opts(true, false));
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let content = b"hello world";
        let results = search(content, "", &opts(true, false));
        assert!(results.is_empty());
    }

    #[test]
    fn test_multiple_per_line() {
        let content = b"aaa aaa aaa";
        let results = search(content, "aaa", &opts(true, false));
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].col, 0);
        assert_eq!(results[1].col, 4);
        assert_eq!(results[2].col, 8);
    }

    #[test]
    fn test_column_tracking() {
        let content = b"the quick brown fox";
        let results = search(content, "fox", &opts(true, false));
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].col, 16);
    }
}
