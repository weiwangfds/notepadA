use std::fs::File;
use std::path::Path;

use memmap2::Mmap;

/// A memory-mapped file wrapper that provides zero-copy read access.
pub struct FileMapper {
    mmap: Mmap,
    file_size: u64,
}

impl FileMapper {
    /// Open and memory-map a file for read access.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let file_size = metadata.len();

        let mmap = unsafe { Mmap::map(&file)? };

        Ok(Self { mmap, file_size })
    }

    /// Returns the raw bytes of the mapped file.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Returns the total file size in bytes.
    #[inline]
    pub fn file_size(&self) -> u64 {
        self.file_size
    }

    /// Returns a slice of bytes in the given range.
    /// Will be used by the Piece Table phase for on-demand chunk reading.
    #[allow(dead_code)]
    #[inline]
    pub fn slice(&self, start: u64, len: u64) -> &[u8] {
        let start = start as usize;
        let len = len as usize;
        let data = self.as_bytes();
        if start >= data.len() {
            return &[];
        }
        let end = (start + len).min(data.len());
        &data[start..end]
    }
}

// Safety: FileMapper only provides read access to the mmap.
unsafe impl Send for FileMapper {}
unsafe impl Sync for FileMapper {}
