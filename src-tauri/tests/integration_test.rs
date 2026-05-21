use std::io::Write;

/// Integration test: verifies the full pipeline from file open to viewport extraction.
/// This mirrors what happens when a user opens a file via the GUI.
fn create_test_file(path: &str, line_count: u64) {
    let mut f = std::fs::File::create(path).expect("create test file");
    for i in 0..line_count {
        writeln!(f, "Line {}: This is test content for NotePadA integration test.", i).unwrap();
    }
}

#[test]
fn test_full_pipeline() {
    let test_path = "/tmp/notepada_integration_test.txt";
    create_test_file(test_path, 5000);

    let metadata = std::fs::metadata(test_path).unwrap();
    println!("Test file: {} ({} bytes)", test_path, metadata.len());

    // 1. Open file via FileMapper (mmap)
    let mapper = app_lib::file::mapper::FileMapper::open(std::path::Path::new(test_path)).expect("mmap open");
    let raw_bytes = mapper.as_bytes();
    assert_eq!(raw_bytes.len(), metadata.len() as usize);

    // 2. Detect encoding
    let enc_info = app_lib::file::encoding::detect(raw_bytes, 64 * 1024);
    assert_eq!(enc_info.encoding, "UTF-8");
    assert!(!enc_info.has_bom);

    // 3. Convert to UTF-8
    let utf8_text = app_lib::file::encoding::convert_to_utf8(raw_bytes, &enc_info.encoding, 0);
    assert!(!utf8_text.is_empty());

    // 4. Build line index
    let mut line_index = app_lib::buffer::line_index::LineIndex::new(utf8_text.as_bytes(), utf8_text.len() as u64);
    line_index.build_full(utf8_text.as_bytes());
    assert_eq!(line_index.total_lines(), 5000);
    assert!(line_index.is_complete());

    // 5. Extract viewport from start
    let lines = line_index.get_lines(utf8_text.as_bytes(), 0, 5);
    assert_eq!(lines.len(), 5);
    assert!(lines[0].starts_with("Line 0:"));
    assert!(lines[4].starts_with("Line 4:"));

    // 6. Extract viewport from middle (uses sparse index)
    let lines = line_index.get_lines(utf8_text.as_bytes(), 2500, 3);
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("Line 2500:"));
    assert!(lines[2].starts_with("Line 2502:"));

    // 7. Extract viewport near end
    let lines = line_index.get_lines(utf8_text.as_bytes(), 4998, 5);
    assert_eq!(lines.len(), 2); // Only lines 4998 and 4999
    assert!(lines[0].starts_with("Line 4998:"));
    assert!(lines[1].starts_with("Line 4999:"));

    // 8. Extract viewport at sparse group boundary
    let lines = line_index.get_lines(utf8_text.as_bytes(), 1024, 2);
    assert!(lines[0].starts_with("Line 1024:"));
    assert!(lines[1].starts_with("Line 1025:"));

    // Cleanup
    std::fs::remove_file(test_path).ok();
}
