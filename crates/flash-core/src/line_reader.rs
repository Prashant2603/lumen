use crate::file_map::FileMap;
use crate::line_index::LineIndex;

/// Reads line slices from a memory-mapped file using a pre-built line index.
pub struct LineReader<'a> {
    data: &'a [u8],
    index: &'a LineIndex,
}

impl<'a> LineReader<'a> {
    pub fn new(file_map: &'a FileMap, index: &'a LineIndex) -> Self {
        Self {
            data: file_map.as_bytes(),
            index,
        }
    }

    /// Get a single line as a string slice, trimming trailing \r\n or \n.
    pub fn get_line(&self, line_num: usize) -> Option<&'a str> {
        let (start, end) = self.index.line_range(line_num)?;
        let start = start as usize;
        let end = end as usize;
        let slice = &self.data[start..end];
        // Trim trailing newline characters
        let trimmed = strip_line_ending(slice);
        // Lossy UTF-8: replace invalid bytes. For performance we try from_utf8 first.
        match std::str::from_utf8(trimmed) {
            Ok(s) => Some(s),
            Err(_) => None, // skip non-utf8 lines
        }
    }

    /// Get a range of lines. Returns up to `count` lines starting at `start`.
    pub fn get_lines(&self, start: usize, count: usize) -> Vec<(usize, &'a str)> {
        let total = self.index.line_count();
        let end = (start + count).min(total);
        let mut result = Vec::with_capacity(end - start);
        for i in start..end {
            if let Some(line) = self.get_line(i) {
                result.push((i, line));
            }
        }
        result
    }

    pub fn line_count(&self) -> usize {
        self.index.line_count()
    }
}

fn strip_line_ending(bytes: &[u8]) -> &[u8] {
    if bytes.ends_with(b"\r\n") {
        &bytes[..bytes.len() - 2]
    } else if bytes.ends_with(b"\n") {
        &bytes[..bytes.len() - 1]
    } else {
        bytes
    }
}
