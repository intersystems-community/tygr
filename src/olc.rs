//! Offset-Line-Character (OLC): converts between byte offsets (as reported by
//! [`Error::pos`](crate::Error::pos)) and `(line, character)` positions,
//! where `character` is a UTF-16 code-unit offset (as required by the
//! Language Server Protocol's `Position.character`).

/// Converts between byte offsets and `(line, character)` positions in a
/// fixed input string.
pub struct OLC {
    // Per line: its starting byte offset, and a (byte, UTF-16) checkpoint
    // after each non-ASCII char. Byte and UTF-16 offsets agree between
    // checkpoints, so lookups never need the input text itself.
    lines: Vec<(usize, Vec<(usize, usize)>)>,
}

impl OLC {
    /// Index `input`'s line boundaries and non-ASCII characters, for later
    /// offset/line-character conversions.
    pub fn new(input: &str) -> Self {
        let mut lines = vec![];
        let mut line_start = 0;
        let mut checkpoints = vec![];
        let mut utf16_in_line = 0;
        for (i, ch) in input.char_indices() {
            if ch == '\n' {
                lines.push((line_start, checkpoints));
                line_start = i + 1;
                checkpoints = vec![];
                utf16_in_line = 0;
                continue;
            }
            let byte_len = ch.len_utf8();
            utf16_in_line += ch.len_utf16();
            if byte_len > 1 {
                checkpoints.push((i - line_start + byte_len, utf16_in_line));
            }
        }
        lines.push((line_start, checkpoints));
        Self { lines }
    }

    /// Convert a byte offset into a zero-indexed `(line, character)` pair,
    /// where `character` is a UTF-16 code-unit offset.
    pub fn offset_to_line_character(&self, offset: usize) -> (usize, usize) {
        let line = self.lines.partition_point(|&(start, _)| start <= offset) - 1;
        let (line_start, checkpoints) = &self.lines[line];
        let byte_in_line = offset - line_start;
        let idx = checkpoints.partition_point(|&(cb, _)| cb <= byte_in_line);
        let character = if idx == 0 {
            byte_in_line
        } else {
            let (cb, cu) = checkpoints[idx - 1];
            cu + (byte_in_line - cb)
        };
        (line, character)
    }

    /// Convert a zero-indexed `(line, character)` pair — `character` a
    /// UTF-16 code-unit offset — into a byte offset.
    pub fn line_character_to_offset(&self, (line, character): (usize, usize)) -> usize {
        let (line_start, checkpoints) = &self.lines[line];
        let idx = checkpoints.partition_point(|&(_, cu)| cu <= character);
        let byte_in_line = if idx == 0 {
            character
        } else {
            let (cb, cu) = checkpoints[idx - 1];
            cb + (character - cu)
        };
        line_start + byte_in_line
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_offset_to_line_char(input: &str, offset: usize) -> (usize, usize) {
        let (mut line, mut character) = (0, 0);
        for (i, ch) in input.char_indices() {
            if i >= offset {
                break;
            }
            if ch == '\n' {
                line += 1;
                character = 0;
            } else {
                character += ch.len_utf16();
            }
        }
        (line, character)
    }

    fn check_all_offsets(input: &str) {
        let olc = OLC::new(input);
        for offset in 0..=input.len() {
            if !input.is_char_boundary(offset) {
                continue;
            }
            let expected = reference_offset_to_line_char(input, offset);
            let actual = olc.offset_to_line_character(offset);
            assert_eq!(
                actual, expected,
                "mismatch at offset {offset} in {input:?}: got {actual:?}, expected {expected:?}"
            );
        }
    }

    #[test]
    fn empty() {
        check_all_offsets("");
    }

    #[test]
    fn single_line() {
        check_all_offsets("hello");
    }

    #[test]
    fn two_lines() {
        check_all_offsets("hello\nworld");
    }

    #[test]
    fn trailing_newline() {
        check_all_offsets("hello\n");
    }

    #[test]
    fn leading_newline() {
        check_all_offsets("\nhello");
    }

    #[test]
    fn consecutive_newlines() {
        check_all_offsets("a\n\nb");
    }

    #[test]
    fn only_newlines() {
        check_all_offsets("\n\n\n");
    }

    #[test]
    fn multiple_lines() {
        check_all_offsets("foo\nbar\nbaz\nqux");
    }

    #[test]
    fn single_char_lines() {
        check_all_offsets("a\nb\nc\nd");
    }

    #[test]
    fn two_byte_utf8_char() {
        // 'é' is 2 bytes in UTF-8, 1 unit in UTF-16.
        check_all_offsets("café");
    }

    #[test]
    fn three_byte_utf8_char() {
        // '華' is 3 bytes in UTF-8, 1 unit in UTF-16.
        check_all_offsets("華語");
    }

    #[test]
    fn four_byte_utf8_char() {
        // '😀' is 4 bytes in UTF-8, a surrogate pair (2 units) in UTF-16.
        check_all_offsets("a😀b");
    }

    #[test]
    fn mixed_non_ascii_across_lines() {
        check_all_offsets("café\n華語\na😀b\nplain");
    }

    #[test]
    fn line_character_to_offset_round_trips() {
        let input = "café\n華語\na😀b\nplain";
        let olc = OLC::new(input);
        for offset in 0..=input.len() {
            if !input.is_char_boundary(offset) {
                continue;
            }
            let lc = olc.offset_to_line_character(offset);
            assert_eq!(olc.line_character_to_offset(lc), offset);
        }
    }
}
