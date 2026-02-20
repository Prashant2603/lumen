use crate::line_index::LineIndex;

/// Reads line slices from a byte source using a pre-built line index.
///
/// The byte source can be a memory-mapped file or any other `&[u8]` —
/// `LineReader` is intentionally decoupled from `FileMap`.
pub struct LineReader<'a> {
    data:  &'a [u8],
    index: &'a LineIndex,
}

impl<'a> LineReader<'a> {
    pub fn new(data: &'a [u8], index: &'a LineIndex) -> Self {
        Self { data, index }
    }

    /// Get a single line as a string slice, trimming trailing `\r\n` / `\n`.
    /// Returns `None` for out-of-range lines or lines with invalid UTF-8.
    pub fn get_line(&self, line_num: usize) -> Option<&'a str> {
        let (start, end) = self.index.line_range(line_num)?;
        let slice = &self.data[start as usize..end as usize];
        let trimmed = strip_line_ending(slice);
        std::str::from_utf8(trimmed).ok()
    }

    /// Get up to `count` lines starting at `start`, as `(line_number, text)` pairs.
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
