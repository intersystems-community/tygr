//! What a derived enum remembers of a field it parsed for one variant, for
//! the later variants that start with the same fields.

/// A field's outcome at the one position every variant sharing it parses
/// it at.
#[doc(hidden)]
pub enum Memo<T> {
    Untried,
    Failed,
    Parsed(T, usize),
}

impl<T> Memo<T> {
    /// Where the field ends, `parse`-ing it only if no earlier variant did;
    /// `None` if it failed, then or now.
    #[inline(always)]
    pub fn end(&mut self, parse: impl FnOnce() -> Option<(T, usize)>) -> Option<usize> {
        if let Memo::Untried = self {
            *self = match parse() {
                Some((value, end)) => Memo::Parsed(value, end),
                None => Memo::Failed,
            };
        }
        match self {
            Memo::Parsed(_, end) => Some(*end),
            _ => None,
        }
    }

    /// The parsed value, for the variant that succeeds.
    #[inline(always)]
    pub fn take(&mut self) -> T {
        match std::mem::replace(self, Memo::Untried) {
            Memo::Parsed(value, _) => value,
            _ => unreachable!("a variant succeeds only past its parsed fields"),
        }
    }
}
