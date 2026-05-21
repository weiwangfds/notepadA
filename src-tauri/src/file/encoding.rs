/// Detected encoding information for a file.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EncodingInfo {
    /// The detected encoding name (e.g., "UTF-8", "GBK", "Shift_JIS")
    pub encoding: String,
    /// Whether a BOM was detected
    pub has_bom: bool,
    /// The detected line ending style
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub enum LineEnding {
    /// Unix-style \n
    LF,
    /// Windows-style \r\n
    CRLF,
    /// Mixed or unknown
    Mixed,
}

/// Detect encoding and line endings from raw file bytes.
/// Only reads the first `max_scan` bytes to keep it fast.
pub fn detect(bytes: &[u8], max_scan: usize) -> EncodingInfo {
    let scan_bytes = &bytes[..bytes.len().min(max_scan)];

    // Check for BOM first
    let (has_bom, encoding_name, bom_len) = detect_bom(bytes);

    let encoding = if has_bom {
        encoding_name.to_string()
    } else {
        // Use chardetng for detection
        let mut detector = chardetng::EncodingDetector::new();
        detector.feed(scan_bytes, true);
        let enc = detector.guess(None, true);
        enc.name().to_string()
    };

    // Convert to UTF-8 for line ending detection
    let utf8_text = convert_to_utf8(bytes, &encoding, bom_len);
    let line_ending = detect_line_ending(&utf8_text);

    EncodingInfo {
        encoding,
        has_bom,
        line_ending,
    }
}

fn detect_bom(bytes: &[u8]) -> (bool, &'static str, usize) {
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return (true, "UTF-8", 3);
    }
    if bytes.len() >= 4 && bytes[0] == 0x00 && bytes[1] == 0x00 && bytes[2] == 0xFE && bytes[3] == 0xFF {
        return (true, "UTF-32BE", 4);
    }
    if bytes.len() >= 4 && bytes[0] == 0xFF && bytes[1] == 0xFE && bytes[2] == 0x00 && bytes[3] == 0x00 {
        return (true, "UTF-32LE", 4);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        return (true, "UTF-16BE", 2);
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        return (true, "UTF-16LE", 2);
    }
    (false, "UTF-8", 0)
}

/// Convert raw bytes to a UTF-8 String using the detected encoding.
pub fn convert_to_utf8(bytes: &[u8], encoding_name: &str, bom_len: usize) -> String {
    let data = if bom_len > 0 && bytes.len() > bom_len {
        &bytes[bom_len..]
    } else {
        bytes
    };

    // Check if already valid UTF-8
    if encoding_name.eq_ignore_ascii_case("UTF-8") {
        String::from_utf8_lossy(data).into_owned()
    } else {
        // Try to find the encoding_rs encoding
        let enc = encoding_rs::Encoding::for_label(encoding_name.as_bytes())
            .unwrap_or(encoding_rs::UTF_8);
        let (cow, _encoding_used, _had_errors) = enc.decode(data);
        cow.into_owned()
    }
}

fn detect_line_ending(text: &str) -> LineEnding {
    let mut has_lf = false;
    let mut has_crlf = false;

    let check_len = text.len().min(65536); // Check first 64KB
    let check_text = &text[..check_len];

    let mut i = 0;
    let bytes = check_text.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\r' {
            if i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
                has_crlf = true;
                i += 2;
                continue;
            }
        } else if bytes[i] == b'\n' {
            has_lf = true;
        }
        i += 1;
    }

    match (has_crlf, has_lf) {
        (true, false) => LineEnding::CRLF,
        (false, true) => LineEnding::LF,
        (true, true) => LineEnding::Mixed,
        (false, false) => LineEnding::LF, // Default to LF for single-line files
    }
}
