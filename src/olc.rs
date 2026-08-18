// Offset-Line-Character (OLC)

pub struct OLC {
    map: Vec<usize>,
}

impl OLC {
    pub fn new(input: &str) -> Self {
        let mut map = vec![];
        for (i, ch) in input.char_indices() {
            if ch == '\n' {
                map.push(i + 1);
            }
        }
        Self { map }
    }

    pub fn offset_to_line_character(&self, offset: usize) -> (usize, usize) {
        let line = self.map.partition_point(|&line_start| line_start <= offset);
        if line == 0 {
            (0, offset)
        } else {
            (line, offset - self.map[line - 1])
        }
    }

    pub fn line_character_to_offset(&self, (line, character): (usize, usize)) -> usize {
        if line == 0 {
            character
        } else {
            character + self.map[line - 1]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_offset_to_line_char(input: &str, offset: usize) -> (usize, usize) {
        let (mut line, mut character) = (0, 0);
        for ch in input.chars().take(offset) {
            if ch == '\n' {
                line += 1;
                character = 0;
            } else {
                character += 1;
            }
        }
        (line, character)
    }

    fn check_all_offsets(input: &str) {
        let olc = OLC::new(input);
        for offset in 0..=input.len() {
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
}
