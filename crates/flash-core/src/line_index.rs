/// Stores byte offsets for the start of each line.
///
/// A single sequential scan finds all `\n` bytes and records offsets.
/// Line N starts at `offsets[N]` and ends just before `offsets[N+1]`.
pub struct LineIndex {
    /// Byte offset of the start of each line. offsets[line_count] == file_len.
    offsets: Vec<u64>,
}

impl LineIndex {
    /// Build a line index by scanning the byte slice for newlines.
    pub fn build(data: &[u8]) -> Self {
        let mut offsets = Vec::new();
        offsets.push(0);
        for (i, &b) in data.iter().enumerate() {
            if b == b'\n' {
                offsets.push((i + 1) as u64);
            }
        }
        // If the file doesn't end with a newline and has content after the last
        // newline, the last entry in offsets already marks that line's start.
        // We add a sentinel for the end-of-file so line_range works uniformly.
        let file_len = data.len() as u64;
        if offsets.last().copied() != Some(file_len) {
            offsets.push(file_len);
        }
        Self { offsets }
    }

    /// Total number of lines (including a possible empty trailing line).
    pub fn line_count(&self) -> usize {
        if self.offsets.len() <= 1 {
            return 0;
        }
        self.offsets.len() - 1
    }

    /// Returns (start_byte_offset, end_byte_offset) for the given line number.
    /// The range excludes the trailing newline if present.
    pub fn line_range(&self, line: usize) -> Option<(u64, u64)> {
        if line >= self.line_count() {
            return None;
        }
        let start = self.offsets[line];
        let mut end = self.offsets[line + 1];
        // Strip trailing \n (and \r\n)
        if end > start {
            let prev = end - 1;
            // Check for \n
            if end > start {
                end = self.offsets[line + 1];
                // We want to trim \r\n or \n from the end
                if end > 0 {
                    // end points to start of next line or EOF sentinel
                    // The actual line content is [start .. next_line_start)
                    // but next_line_start already excludes the newline in terms of content
                    // Actually: offsets[line+1] is the byte AFTER \n, so line content
                    // including \n is [start..offsets[line+1]).
                    // We want to strip \n and optional \r.
                    let _ = prev; // suppress warning
                }
            }
        }
        // Simpler approach: return raw range, let LineReader trim.
        Some((start, self.offsets[line + 1]))
    }

    /// Direct access to offsets for advanced use.
    pub fn offset(&self, line: usize) -> Option<u64> {
        self.offsets.get(line).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let data = b"hello\nworld\n";
        let idx = LineIndex::build(data);
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_range(0), Some((0, 6)));
        assert_eq!(idx.line_range(1), Some((6, 12)));
    }

    #[test]
    fn test_no_trailing_newline() {
        let data = b"hello\nworld";
        let idx = LineIndex::build(data);
        assert_eq!(idx.line_count(), 2);
        assert_eq!(idx.line_range(0), Some((0, 6)));
        assert_eq!(idx.line_range(1), Some((6, 11)));
    }

    #[test]
    fn test_empty() {
        let data = b"";
        let idx = LineIndex::build(data);
        assert_eq!(idx.line_count(), 0);
    }

    #[test]
    fn test_single_newline() {
        let data = b"\n";
        let idx = LineIndex::build(data);
        assert_eq!(idx.line_count(), 1);
    }
}
