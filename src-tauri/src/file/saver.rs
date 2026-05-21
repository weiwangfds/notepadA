use std::fs;
use std::io::Write;
use std::path::Path;

/// Save content to a file atomically using a temp file + rename.
///
/// This ensures that if the process crashes during write, the original file
/// is not corrupted. The write goes to a temporary file first, then is
/// atomically renamed over the original.
pub fn save_atomic(path: &Path, content: &[u8]) -> Result<(), String> {
    let tmp_path = path.with_extension("notepada.tmp");

    // Write to temp file
    let mut file = fs::File::create(&tmp_path)
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    file.write_all(content)
        .map_err(|e| format!("Failed to write content: {}", e))?;

    file.sync_all()
        .map_err(|e| format!("Failed to sync file: {}", e))?;

    drop(file);

    // Atomic rename
    fs::rename(&tmp_path, path)
        .map_err(|e| format!("Failed to rename temp file: {}", e))?;

    Ok(())
}
