//! Byte offsets ⇄ `(line, character)` positions.

/// Converts between byte offsets and `(line, character)` in one input,
/// `character` in UTF-16 code units (as in the Language Server Protocol).
/// Lines end at `\n`, `\r\n` or `\r`.
#[derive(Debug, Clone)]
pub struct OLC {
    lines: Vec<Line>,
}

/// One line: its start and end offsets (before its line break), and each
/// non-ASCII char's start in bytes and UTF-16 units into the line, with its
/// length in bytes. Between non-ASCII chars, bytes and units agree.
#[derive(Debug, Clone)]
struct Line {
    start: usize,
    end: usize,
    wide: Vec<(usize, usize, usize)>,
}

impl OLC {
    /// Index `input`.
    pub fn new(input: &str) -> Self {
        let mut lines = vec![];
        let mut line = Line::new(0);
        let mut unit = 0;
        let mut chars = input.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            if ch == '\n' || ch == '\r' {
                let crlf = ch == '\r' && chars.next_if(|&(_, next)| next == '\n').is_some();
                line.end = i;
                lines.push(std::mem::replace(
                    &mut line,
                    Line::new(i + 1 + crlf as usize),
                ));
                unit = 0;
                continue;
            }
            if !ch.is_ascii() {
                line.wide.push((i - line.start, unit, ch.len_utf8()));
            }
            unit += ch.len_utf16();
        }
        line.end = input.len();
        lines.push(line);
        Self { lines }
    }

    /// Byte offset → zero-indexed `(line, character)`. An offset inside a
    /// char or a line break counts as that char's start or the line's end,
    /// and any offset past the input as its end.
    pub fn offset_to_line_character(&self, offset: usize) -> (usize, usize) {
        let index = self.lines.partition_point(|line| line.start <= offset) - 1;
        let line = &self.lines[index];
        let byte = offset.min(line.end) - line.start;
        let k = line.wide.partition_point(|&(start, _, _)| start <= byte);
        let character = match line.wide[..k].last().copied() {
            None => byte,
            Some((start, unit, len)) if byte < start + len => unit,
            Some((start, unit, len)) => unit + units(len) + (byte - start - len),
        };
        (index, character)
    }

    /// Zero-indexed `(line, character)` → byte offset. As in the Language
    /// Server Protocol, a character past the line's end means its end, and a
    /// line past the last means the input's end; a character inside a
    /// surrogate pair counts as the pair's start.
    pub fn line_character_to_offset(&self, (line, character): (usize, usize)) -> usize {
        let Some(line) = self.lines.get(line) else {
            return self.lines.last().map_or(0, |line| line.end);
        };
        let k = line.wide.partition_point(|&(_, unit, _)| unit <= character);
        let byte = match line.wide[..k].last().copied() {
            None => character,
            Some((start, unit, len)) if character < unit + units(len) => start,
            Some((start, unit, len)) => (start + len).saturating_add(character - unit - units(len)),
        };
        line.start.saturating_add(byte).min(line.end)
    }
}

impl Line {
    fn new(start: usize) -> Self {
        Line {
            start,
            end: start,
            wide: vec![],
        }
    }
}

/// The UTF-16 units of a char that's `len` bytes in UTF-8.
fn units(len: usize) -> usize {
    if len == 4 { 2 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_offset_to_line_character(input: &str, offset: usize) -> (usize, usize) {
        let (mut line, mut character) = (0, 0);
        let mut chars = input.char_indices().peekable();
        while let Some((i, ch)) = chars.next() {
            if i + ch.len_utf8() > offset {
                break;
            }
            match ch {
                '\r' if chars.peek().is_some_and(|&(_, next)| next == '\n') => {
                    // Between `\r` and `\n` is still the line's end.
                    if i + 1 >= offset {
                        break;
                    }
                    chars.next();
                    line += 1;
                    character = 0;
                }
                '\n' | '\r' => {
                    line += 1;
                    character = 0;
                }
                _ => character += ch.len_utf16(),
            }
        }
        (line, character)
    }

    fn check_all_offsets(input: &str) {
        let olc = OLC::new(input);
        for offset in 0..=input.len() {
            let expected = reference_offset_to_line_character(input, offset);
            let actual = olc.offset_to_line_character(offset);
            assert_eq!(actual, expected, "offset {offset} in {input:?}");
            if !input.is_char_boundary(offset) {
                continue;
            }
            let inside_crlf =
                offset > 0 && input[..offset].ends_with('\r') && input[offset..].starts_with('\n');
            if !inside_crlf {
                assert_eq!(
                    olc.line_character_to_offset(actual),
                    offset,
                    "round trip of {offset} in {input:?}"
                );
            }
        }
    }

    #[test]
    fn ascii() {
        for input in [
            "",
            "hello",
            "hello\nworld",
            "hello\n",
            "\nhello",
            "a\n\nb",
            "\n\n\n",
            "foo\nbar\nbaz\nqux",
            "a\nb\nc\nd",
        ] {
            check_all_offsets(input);
        }
    }

    #[test]
    fn non_ascii() {
        // 'é' is 2 bytes and 1 unit, '平' 3 bytes and 1 unit, '😀' 4 bytes and a surrogate pair.
        for input in ["café", "平安", "a😀b", "café\n平安\na😀b\nplain"] {
            check_all_offsets(input);
        }
    }

    #[test]
    fn crlf_and_cr_break_lines() {
        check_all_offsets("a\r\nb\rc\n\r\n😀\r");
        let olc = OLC::new("a\r\nb\rc");
        assert_eq!(olc.offset_to_line_character(3), (1, 0));
        assert_eq!(olc.offset_to_line_character(5), (2, 0));
        assert_eq!(olc.line_character_to_offset((2, 0)), 5);
    }

    #[test]
    fn out_of_range_positions_clamp() {
        let olc = OLC::new("ab\r\ncd");
        assert_eq!(olc.line_character_to_offset((0, 9)), 2);
        assert_eq!(olc.line_character_to_offset((1, usize::MAX)), 6);
        assert_eq!(olc.line_character_to_offset((9, 0)), 6);
        assert_eq!(olc.offset_to_line_character(99), (1, 2));
        assert_eq!(OLC::new("😀").line_character_to_offset((0, usize::MAX)), 4);
    }

    #[test]
    fn inside_a_char_counts_as_its_start() {
        let olc = OLC::new("a😀b");
        assert_eq!(olc.line_character_to_offset((0, 2)), 1);
        assert_eq!(olc.offset_to_line_character(2), (0, 1));
        assert_eq!(olc.line_character_to_offset((0, 3)), 5);
    }
}
