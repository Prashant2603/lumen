use memmap2::Mmap;
use std::fs::File;
use std::io;
use std::path::Path;
use std::sync::Arc;

/// Read-only memory-mapped file wrapper.
///
/// The inner `Mmap` is stored behind an `Arc` so that the same mapping can be
/// shared cheaply with the search worker thread — no file data is ever copied.
pub struct FileMap {
    mmap: Arc<Mmap>,
    len: u64,
}

impl FileMap {
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        // SAFETY: we open read-only and assume no external mutation during the
        // lifetime of this process (standard log-viewer guarantee).
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self { mmap: Arc::new(mmap), len })
    }

    /// Byte slice view of the entire file.
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.mmap
    }

    /// Clone the inner `Arc<Mmap>` for sharing with other threads.
    /// This is O(1) — it just increments the reference count.
    pub fn clone_mmap_arc(&self) -> Arc<Mmap> {
        self.mmap.clone()
    }

    pub fn len(&self) -> u64 { self.len }
    pub fn is_empty(&self) -> bool { self.len == 0 }
}
