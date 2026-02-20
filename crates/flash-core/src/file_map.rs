use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::path::Path;

/// Read-only memory-mapped file wrapper.
pub struct FileMap {
    mmap: Mmap,
    len: u64,
}

impl FileMap {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let metadata = file.metadata()?;
        let len = metadata.len();
        // SAFETY: We open the file read-only and assume no external mutation.
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self { mmap, len })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}
