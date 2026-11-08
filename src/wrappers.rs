//! Operators that keep the match but change the value or the BNF.

use crate::State;
use crate::bnf::{Expr, Rules};
use crate::grammar::{Check, Mode, Parse, Print};
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};
use tygr_derive::Grammar;

/// Unwrap a wrapper grammar to the value it holds.
pub trait IntoInner<T> {
    /// The wrapped value.
    fn into_inner(self) -> T;
}

/// Parses and prints as `T`, but is left out of BNF (e.g. whitespace).
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// type Ws = Hidden<StringOf<IsSpace>>;
/// ```
///
/// Or use `#[tygr(hidden)]`:
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// #[tygr(hidden)]
/// struct Ws(StringOf<IsSpace>);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Grammar)]
#[tygr(hidden)]
pub struct Hidden<T>(T);

impl<T> Deref for Hidden<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Hidden<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> IntoInner<T> for Hidden<T> {
    fn into_inner(self) -> T {
        self.0
    }
}

/// Parses as `T` but keeps only the matched text.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// #[tygr(name = "ws", hidden)]
/// struct Ws(StringOf<IsSpace>);
///
/// # #[derive(Grammar)]
/// # struct Term(StringOf1<IsDigit>);
/// # #[derive(Grammar)]
/// # struct AddOp(StringEq!("+"));
/// // Whitespace stored as string
/// #[derive(Grammar)]
/// #[tygr(name = "expr")]
/// struct Expr(Term, Vec<(Raw<Ws>, AddOp, Raw<Ws>, Term)>);
/// ```
pub struct Raw<T>(pub String, PhantomData<T>);

impl<T> Raw<T> {
    /// Construct a `Raw` from an already-known string.
    pub fn new(s: impl Into<String>) -> Self {
        Raw(s.into(), PhantomData)
    }

    /// The matched text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<T: Parse> Parse for Raw<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let ((), end) = T::parse_at::<Check>(input, pos, state)?;
        Some((M::build(|_| Raw::from(text(input, pos, end))), end))
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

// Not derived: only the text matters, so no bounds on `T`.

impl<T> Print for Raw<T> {
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.as_bytes());
    }
}

impl<T> fmt::Debug for Raw<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Raw").field(&self.0).finish()
    }
}

impl<T> Clone for Raw<T> {
    fn clone(&self) -> Self {
        Raw(self.0.clone(), PhantomData)
    }
}

impl<T> PartialEq for Raw<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Raw<T> {}

impl<T> std::hash::Hash for Raw<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> Deref for Raw<T> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl<T> AsRef<str> for Raw<T> {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl<T> IntoInner<String> for Raw<T> {
    fn into_inner(self) -> String {
        self.0
    }
}

impl<T> fmt::Display for Raw<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl<T> From<&str> for Raw<T> {
    fn from(s: &str) -> Self {
        Raw(s.to_string(), PhantomData)
    }
}

impl<T> From<String> for Raw<T> {
    fn from(s: String) -> Self {
        Raw(s, PhantomData)
    }
}

/// Parses as `T` and keeps its `start..end` byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ranged<T> {
    /// Start byte offset.
    pub start: usize,
    ranged: T,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl<T> IntoInner<T> for Ranged<T> {
    fn into_inner(self) -> T {
        self.ranged
    }
}

impl<T> Deref for Ranged<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.ranged
    }
}

impl<T> Ranged<T> {
    /// The `start..end` byte range.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Construct from an already-known range.
    pub fn new(start: usize, ranged: T, end: usize) -> Self {
        Ranged { start, ranged, end }
    }

    /// Apply `f` to the ranged value, keeping the same range.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Ranged<U> {
        Ranged {
            start: self.start,
            ranged: f(self.ranged),
            end: self.end,
        }
    }

    /// Borrow the ranged value, keeping the same range.
    pub fn as_ref(&self) -> Ranged<&T> {
        Ranged {
            start: self.start,
            ranged: &self.ranged,
            end: self.end,
        }
    }
}

impl<T> Ranged<Option<T>> {
    /// Swap `Ranged<Option<T>>` for `Option<Ranged<T>>`.
    pub fn transpose(self) -> Option<Ranged<T>> {
        Some(Ranged::new(self.start, self.ranged?, self.end))
    }
}

impl<T, E> Ranged<Result<T, E>> {
    /// Swap `Ranged<Result<T, E>>` for `Result<Ranged<T>, E>`.
    pub fn transpose(self) -> Result<Ranged<T>, E> {
        Ok(Ranged::new(self.start, self.ranged?, self.end))
    }
}

impl<T: Parse> Parse for Ranged<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let (ranged, end) = T::parse_at::<M>(input, pos, state)?;
        Some((M::map(ranged, |ranged| Ranged::new(pos, ranged, end)), end))
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Ranged<T> {
    fn print_to(&self, buf: &mut Vec<u8>) {
        self.ranged.print_to(buf);
    }
}

impl<T: Parse> Parse for Box<T> {
    type First = T::First;

    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let (value, end) = T::parse_at::<M>(input, pos, state)?;
        Some((M::map(value, Box::new), end))
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Box<T> {
    fn print_to(&self, buf: &mut Vec<u8>) {
        (**self).print_to(buf);
    }
}

/// Matches `T` and discards it; prints nothing.
impl<T: Parse> Parse for PhantomData<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let ((), end) = T::parse_at::<Check>(input, pos, state)?;
        Some((M::build(|_| PhantomData), end))
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T> Print for PhantomData<T> {
    fn print_to(&self, _buf: &mut Vec<u8>) {}
}

/// The text of `input[pos..end]`.
#[inline(always)]
pub(crate) fn text(input: &[u8], pos: usize, end: usize) -> &str {
    // SAFETY: see the invariant on `Parse::parse_at`.
    unsafe { std::str::from_utf8_unchecked(&input[pos..end]) }
}
