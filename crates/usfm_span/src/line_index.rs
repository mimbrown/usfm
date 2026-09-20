use std::fmt::{self, Debug};

/// Line and column lookup for one source string.
///
/// A [`Span`](crate::Span) is a byte range, which is what the parser wants and
/// what a person reading an error message does not. `LineIndex` records every
/// line start once, so turning any number of offsets into `line:column` costs a
/// binary search plus a character count inside the one line, instead of a scan
/// from the start of the file per offset.
///
/// ```
/// use usfm_span::LineIndex;
///
/// let index = LineIndex::new("\\id GEN\n\\c 1\n");
/// assert_eq!(index.line_col(0), (1, 1));
/// assert_eq!(index.line_col(8), (2, 1));
/// ```
#[derive(Clone)]
pub struct LineIndex<'a> {
    source: &'a str,
    /// Byte offset of the first byte of each line. Always starts with `0`, so
    /// it is never empty and line `n` starts at `line_starts[n - 1]`.
    line_starts: Vec<u32>,
}

impl<'a> LineIndex<'a> {
    /// Record the line starts of `source`.
    ///
    /// A line ends at `\n`; a `\r\n` ending is one break, because the `\r`
    /// belongs to the line it ends and never starts the next one.
    pub fn new(source: &'a str) -> Self {
        let mut line_starts = Vec::with_capacity(source.len() / 32 + 1);
        line_starts.push(0);
        for (offset, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(offset as u32 + 1);
            }
        }
        Self {
            source,
            line_starts,
        }
    }

    /// Convert a byte offset into a 1-based `(line, column)` pair, counting
    /// columns in characters.
    ///
    /// An offset past the end of the source maps to the last position, and one
    /// inside a multi-byte character maps to that character's own position
    /// rather than panicking.
    ///
    /// ```
    /// use usfm_span::LineIndex;
    ///
    /// let index = LineIndex::new("ab\ncdé\nf");
    /// assert_eq!(index.line_col(3), (2, 1));
    /// assert_eq!(index.line_col(7), (2, 4)); // after the two-byte é
    /// assert_eq!(index.line_col(100), (3, 2)); // clamped to the end
    /// ```
    pub fn line_col(&self, offset: u32) -> (usize, usize) {
        let (line, before) = self.line_and_text_before(offset);
        (line, before.chars().count() + 1)
    }

    /// Convert a byte offset into a 1-based `(line, column)` pair, counting
    /// columns in **UTF-16 code units**.
    ///
    /// The Language Server Protocol measures a `Position.character` in UTF-16
    /// code units by default, so `apps/usfm_language_server` maps a [`Span`]
    /// through this rather than through [`line_col`](Self::line_col): a
    /// character outside the Basic Multilingual Plane (an emoji, say) is one
    /// column there and two here. Both are 1-based, like every other position
    /// this crate produces; a caller that wants LSP's 0-based `Position`
    /// subtracts one from each.
    ///
    /// Out-of-range and mid-character offsets behave exactly as
    /// [`line_col`](Self::line_col)'s do.
    ///
    /// ```
    /// use usfm_span::LineIndex;
    ///
    /// let index = LineIndex::new("é😀x");
    /// assert_eq!(index.line_col(2), (1, 2)); // after é: one character
    /// assert_eq!(index.line_col_utf16(2), (1, 2)); // é is one UTF-16 unit
    /// assert_eq!(index.line_col(6), (1, 3)); // after the emoji
    /// assert_eq!(index.line_col_utf16(6), (1, 4)); // which is a surrogate pair
    /// ```
    pub fn line_col_utf16(&self, offset: u32) -> (usize, usize) {
        let (line, before) = self.line_and_text_before(offset);
        (line, before.chars().map(char::len_utf16).sum::<usize>() + 1)
    }

    /// The 1-based line `offset` falls on, and the text from the start of that
    /// line up to `offset` — the part both column counts share.
    ///
    /// An offset past the end of the source is clamped to the end, and one
    /// inside a multi-byte character is moved back to that character's start,
    /// so the slice is always a valid one.
    fn line_and_text_before(&self, offset: u32) -> (usize, &str) {
        let mut offset = (offset as usize).min(self.source.len());
        while !self.source.is_char_boundary(offset) {
            offset -= 1;
        }
        // The first line start greater than `offset` is the line after this
        // one, and `line_starts[0]` is 0, so this is at least 1.
        let line = self
            .line_starts
            .partition_point(|&start| start as usize <= offset);
        let line_start = self.line_starts[line - 1] as usize;
        (line, &self.source[line_start..offset])
    }

    /// The number of lines, counting a trailing newline as ending the last
    /// line rather than starting another.
    pub fn line_count(&self) -> usize {
        if self.source.ends_with('\n') {
            self.line_starts.len() - 1
        } else {
            self.line_starts.len()
        }
    }
}

// The source can be a whole book; printing it in full is never what a `Debug`
// of an index wants, so this prints its length and the number of lines.
impl Debug for LineIndex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LineIndex")
            .field("len", &self.source.len())
            .field("lines", &self.line_starts.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    #[test]
    fn lf_source() {
        let source = "ab\ncdé\nf";
        let index = LineIndex::new(source);
        assert_eq!(index.line_col(0), (1, 1));
        assert_eq!(index.line_col(2), (1, 3));
        assert_eq!(index.line_col(3), (2, 1));
        // `é` is two bytes and one column.
        assert_eq!(index.line_col(5), (2, 3));
        assert_eq!(index.line_col(7), (2, 4));
        assert_eq!(index.line_col(8), (3, 1));
        assert_eq!(index.line_count(), 3);
    }

    #[test]
    fn crlf_source() {
        let source = "ab\r\ncd\r\n";
        let index = LineIndex::new(source);
        // `\r\n` is one break: the next line starts at column 1, and the `\r`
        // is not a column of it.
        assert_eq!(index.line_col(0), (1, 1));
        assert_eq!(index.line_col(4), (2, 1));
        assert_eq!(index.line_col(5), (2, 2));
        // The `\r` is still the last position of the line it ends.
        assert_eq!(index.line_col(2), (1, 3));
        assert_eq!(index.line_count(), 2);
    }

    #[test]
    fn multi_byte_line() {
        // Four characters, seven bytes: é (2), 中 (3), a (1), b (1).
        let source = "\\v 1 é中ab";
        let index = LineIndex::new(source);
        assert_eq!(index.line_col(5), (1, 6)); // é
        assert_eq!(index.line_col(7), (1, 7)); // 中
        assert_eq!(index.line_col(10), (1, 8)); // a
        assert_eq!(index.line_col(11), (1, 9)); // b
        // Inside `中`, not on a boundary: that character's own position.
        assert_eq!(index.line_col(8), (1, 7));
    }

    #[test]
    fn offset_at_eof() {
        let source = "ab\ncd\n";
        let index = LineIndex::new(source);
        // Exactly at the end, after the trailing newline.
        assert_eq!(index.line_col(6), (3, 1));
        // Past the end clamps to the same position.
        assert_eq!(index.line_col(999), (3, 1));
    }

    #[test]
    fn last_line_without_trailing_newline() {
        let source = "ab\ncdef";
        let index = LineIndex::new(source);
        assert_eq!(index.line_col(5), (2, 3)); // middle of the last line
        assert_eq!(index.line_col(7), (2, 5)); // its end
        assert_eq!(index.line_col(999), (2, 5));
        assert_eq!(index.line_count(), 2);
    }

    #[test]
    fn empty_source() {
        let index = LineIndex::new("");
        assert_eq!(index.line_col(0), (1, 1));
        assert_eq!(index.line_col(10), (1, 1));
        assert_eq!(index.line_count(), 1);
    }

    #[test]
    fn utf16_columns_count_code_units() {
        // `é` is two bytes and one UTF-16 unit; `😀` is four bytes and two
        // (a surrogate pair); `中` is three bytes and one.
        let source = "\\v 1 é😀中b\nplain\n";
        let index = LineIndex::new(source);
        // Up to the `é` the two counts agree: ASCII is one of everything.
        assert_eq!(index.line_col(5), (1, 6));
        assert_eq!(index.line_col_utf16(5), (1, 6));
        // After `é`.
        assert_eq!(index.line_col(7), (1, 7));
        assert_eq!(index.line_col_utf16(7), (1, 7));
        // After the emoji: one character, two code units.
        assert_eq!(index.line_col(11), (1, 8));
        assert_eq!(index.line_col_utf16(11), (1, 9));
        // After `中`.
        assert_eq!(index.line_col(14), (1, 9));
        assert_eq!(index.line_col_utf16(14), (1, 10));
        // The next line starts over at column 1 either way.
        assert_eq!(index.line_col_utf16(16), (2, 1));
        assert_eq!(index.line_col_utf16(18), (2, 3));
    }

    #[test]
    fn utf16_columns_clamp_and_snap_like_line_col() {
        let source = "ab\né😀\n";
        let index = LineIndex::new(source);
        // Inside the emoji, not on a character boundary: the character's own
        // position, as `line_col` does.
        assert_eq!(index.line_col_utf16(7), (2, 2));
        assert_eq!(index.line_col_utf16(8), (2, 2));
        // Exactly at the end, and past it.
        assert_eq!(index.line_col_utf16(11), (3, 1));
        assert_eq!(index.line_col_utf16(999), (3, 1));
        // An empty source has nowhere else to be.
        assert_eq!(LineIndex::new("").line_col_utf16(7), (1, 1));
    }

    #[test]
    fn every_offset_matches_a_scan_from_the_start() {
        // The reference implementation this replaced, kept as an oracle.
        fn scan(source: &str, offset: u32) -> (usize, usize) {
            let offset = (offset as usize).min(source.len());
            let before = &source[..offset];
            let line = before.matches('\n').count() + 1;
            let line_start = before.rfind('\n').map_or(0, |i| i + 1);
            let col = before[line_start..].chars().count() + 1;
            (line, col)
        }

        for source in [
            "",
            "\n",
            "ab\ncdé\nf",
            "ab\r\ncd\r\n",
            "\\id GEN\n\\c 1\n\\v 1 In the beginning\n\n\\v 2 é中\n",
        ] {
            let index = LineIndex::new(source);
            for offset in 0..=source.len() as u32 + 3 {
                if (offset as usize) <= source.len()
                    && !source.is_char_boundary(offset as usize)
                {
                    continue; // the scan would panic; the index clamps
                }
                assert_eq!(
                    index.line_col(offset),
                    scan(source, offset),
                    "offset {offset} in {source:?}"
                );
            }
        }
    }
}
