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
    /// line break counts as the line's end. Past the input, or inside a char,
    /// it's [`OutOfRange`], bounded by the input's end or the char's start.
    pub fn offset_to_line_character(
        &self,
        offset: usize,
    ) -> Result<(usize, usize), OutOfRange<(usize, usize)>> {
        let index = self.lines.partition_point(|line| line.start <= offset) - 1;
        let line = &self.lines[index];
        if offset > line.end {
            // Inside the line's break, unless this is the last line.
            let end = (index, line.units_at(line.len()));
            return if index == self.lines.len() - 1 {
                Err(OutOfRange {
                    bound: end,
                    excess: offset - line.end,
                })
            } else {
                Ok(end)
            };
        }
        let byte = offset - line.start;
        if let Some((start, unit, len)) = line.wide_at(byte)
            && byte > start
            && byte < start + len
        {
            return Err(OutOfRange {
                bound: (index, unit),
                excess: byte - start,
            });
        }
        Ok((index, line.units_at(byte)))
    }

    /// Zero-indexed `(line, character)` → byte offset. Past the last line,
    /// past the line's end, or inside a surrogate pair, it's [`OutOfRange`],
    /// bounded by the input's end, the line's end or the pair's start:
    /// `unwrap_or_else(|e| e.bound)` clamps as the Language Server Protocol
    /// specifies.
    pub fn line_character_to_offset(
        &self,
        (line, character): (usize, usize),
    ) -> Result<usize, OutOfRange<usize>> {
        let last = self.lines.len() - 1;
        let Some(line) = self.lines.get(line) else {
            return Err(OutOfRange {
                bound: self.lines[last].end,
                excess: line - last,
            });
        };
        let k = line.wide.partition_point(|&(_, unit, _)| unit <= character);
        let byte = match line.wide[..k].last().copied() {
            None => character,
            Some((start, unit, _)) if character == unit => start,
            Some((start, unit, len)) if character < unit + units(len) => {
                return Err(OutOfRange {
                    bound: line.start + start,
                    excess: character - unit,
                });
            }
            Some((start, unit, len)) => (start + len).saturating_add(character - unit - units(len)),
        };
        if byte > line.len() {
            return Err(OutOfRange {
                bound: line.end,
                excess: character - line.units_at(line.len()),
            });
        }
        Ok(line.start + byte)
    }
}

/// A position outside the input, or inside a char or surrogate pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfRange<T> {
    /// The nearest valid result: the line's or the input's end, or the start
    /// of the char the position fell inside.
    pub bound: T,
    /// How far past `bound` the position was, in its own unit: bytes for an
    /// offset, lines for a line, UTF-16 units for a character.
    pub excess: usize,
}

impl Line {
    fn new(start: usize) -> Self {
        Line {
            start,
            end: start,
            wide: vec![],
        }
    }

    fn len(&self) -> usize {
        self.end - self.start
    }

    /// The last non-ASCII char starting at or before `byte` into the line.
    fn wide_at(&self, byte: usize) -> Option<(usize, usize, usize)> {
        let k = self.wide.partition_point(|&(start, _, _)| start <= byte);
        self.wide[..k].last().copied()
    }

    /// The UTF-16 units before `byte` into the line; inside a char, before
    /// that char.
    fn units_at(&self, byte: usize) -> usize {
        match self.wide_at(byte) {
            None => byte,
            Some((start, unit, len)) if byte < start + len => unit,
            Some((start, unit, len)) => unit + units(len) + (byte - start - len),
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
            if !input.is_char_boundary(offset) {
                let start = (0..offset)
                    .rev()
                    .find(|&b| input.is_char_boundary(b))
                    .unwrap();
                let inside = OutOfRange {
                    bound: expected,
                    excess: offset - start,
                };
                assert_eq!(actual, Err(inside), "offset {offset} in {input:?}");
                continue;
            }
            assert_eq!(actual, Ok(expected), "offset {offset} in {input:?}");
            let inside_crlf =
                offset > 0 && input[..offset].ends_with('\r') && input[offset..].starts_with('\n');
            if !inside_crlf {
                assert_eq!(
                    olc.line_character_to_offset(expected),
                    Ok(offset),
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
        assert_eq!(olc.offset_to_line_character(3), Ok((1, 0)));
        assert_eq!(olc.offset_to_line_character(5), Ok((2, 0)));
        assert_eq!(olc.line_character_to_offset((2, 0)), Ok(5));
    }

    fn out_of_range<T>(bound: T, excess: usize) -> OutOfRange<T> {
        OutOfRange { bound, excess }
    }

    #[test]
    fn out_of_range_positions_report_the_bound_and_the_excess() {
        let olc = OLC::new("ab\r\ncd");
        assert_eq!(
            olc.line_character_to_offset((0, 9)),
            Err(out_of_range(2, 7))
        );
        assert_eq!(
            olc.line_character_to_offset((1, usize::MAX)),
            Err(out_of_range(6, usize::MAX - 2))
        );
        assert_eq!(
            olc.line_character_to_offset((9, 0)),
            Err(out_of_range(6, 8))
        );
        assert_eq!(
            olc.offset_to_line_character(99),
            Err(out_of_range((1, 2), 93))
        );
        assert_eq!(
            OLC::new("😀").line_character_to_offset((0, usize::MAX)),
            Err(out_of_range(4, usize::MAX - 2))
        );
    }

    #[test]
    fn inside_a_char_reports_its_start() {
        let olc = OLC::new("a😀b");
        assert_eq!(
            olc.line_character_to_offset((0, 2)),
            Err(out_of_range(1, 1))
        );
        assert_eq!(
            olc.offset_to_line_character(2),
            Err(out_of_range((0, 1), 1))
        );
        assert_eq!(olc.line_character_to_offset((0, 3)), Ok(5));
    }
}
