//! What a derived enum remembers of a field it parsed for one variant, for
//! the later variants that start with the same fields.

/// A field's outcome at the position every variant sharing it parses it at:
/// not yet tried, failed, or parsed to an end.
#[doc(hidden)]
pub enum Memo<T> {
    Untried,
    Failed,
    Parsed(T, usize),
}

impl<T> Memo<T> {
    /// The parsed value, for the variant that succeeds.
    #[inline(always)]
    pub fn take(&mut self) -> T {
        match std::mem::replace(self, Memo::Untried) {
            Memo::Parsed(value, _) => value,
            _ => unreachable!("a variant succeeds only past its parsed fields"),
        }
    }
}
