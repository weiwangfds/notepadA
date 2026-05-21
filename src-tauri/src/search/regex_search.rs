use super::{SearchMatch, SearchOptions};

/// Search for a regex pattern in `content`.
/// Returns all matches with line/column information.
pub fn search_regex(content: &[u8], pattern: &str, options: &SearchOptions) -> Vec<SearchMatch> {
    if pattern.is_empty() || content.is_empty() {
        return Vec::new();
    }

    // Build regex
    let regex_str = if options.whole_word {
        format!(r"\b{}\b", pattern)
    } else {
        pattern.to_string()
    };

    let re = match regex::RegexBuilder::new(&regex_str)
        .case_insensitive(!options.case_sensitive)
        .build()
    {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let text = match std::str::from_utf8(content) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut matches = Vec::new();
    let mut line = 0u64;
    let mut line_start = 0usize;

    for mat in re.find_iter(text) {
        // Calculate line and column for this match
        while line_start < mat.start() {
            if let Some(nl) = memchr::memchr(b'\n', &content[line_start..mat.start()]) {
                line_start += nl + 1;
                line += 1;
            } else {
                break;
            }
        }

        let col = mat.start() - line_start;
        let matched_text = mat.as_str().to_string();

        matches.push(SearchMatch {
            line,
            col: col as u32,
            length: matched_text.len() as u32,
            text: matched_text,
        });
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(case_sensitive: bool, whole_word: bool) -> SearchOptions {
        SearchOptions {
            case_sensitive,
            whole_word,
            regex: true,
        }
    }

    #[test]
    fn test_basic_regex() {
        let content = b"hello123 world\nfoo456 bar";
        let results = search_regex(content, r"\d+", &opts(true, false));
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].text, "123");
        assert_eq!(results[1].text, "456");
    }

    #[test]
    fn test_case_insensitive_regex() {
        let content = b"Hello World\nhello rust";
        let results = search_regex(content, "hello", &opts(false, false));
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_word_boundary_regex() {
        let content = b"hello world\nhelloworld";
        let results = search_regex(content, "hello", &opts(true, true));
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_invalid_regex() {
        let content = b"hello world";
        let results = search_regex(content, "[invalid", &opts(true, false));
        assert!(results.is_empty());
    }

    #[test]
    fn test_complex_pattern() {
        let content = b"foo@bar.com\nnot-an-email\nbaz@qux.org";
        let results = search_regex(content, r"\w+@\w+\.\w+", &opts(true, false));
        assert_eq!(results.len(), 2);
    }
}
