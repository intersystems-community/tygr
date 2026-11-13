//! Wrappers that keep the match but change the value or the EBNF.

use crate::State;
use crate::bnf::{Expr, Rules};
use crate::grammar::{Mode, Parse, Print};
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut, Range};
use tygr_derive::Grammar;

/// Unwrap a wrapper grammar to the value it holds.
pub trait IntoInner<T> {
    /// The wrapped value.
    fn into_inner(self) -> T;
}

/// Parses and prints as `T`, but is left out of EBNF (e.g. whitespace).
///
/// ```
/// # use tygr::*;
/// # char_class!(Space, "space", |ch| matches!(ch, ' ' | '\t'));
/// type Ws = Hidden<StringOf<Space>>;
/// ```
///
/// Or use `#[tygr(hidden)]`:
///
/// ```
/// # use tygr::*;
/// # char_class!(Space, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// #[tygr(hidden)]
/// struct Ws(StringOf<Space>);
/// ```
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Grammar)]
#[tygr(hidden)]
pub struct Hidden<T>(pub T);

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
/// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
/// #[derive(Grammar)]
/// struct Integer(Raw<(Option<StringEq!("-")>, StringOf1<Digit>)>);
/// assert_eq!(Integer::parse("-42").unwrap().0.as_str(), "-42");
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
        input: &str,
        pos: usize,
        state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let end = T::check_at(input, pos, state)?;
        Some((M::build(|_| Raw::from(text(input, pos, end))), end))
    }

    fn fail_at<M: Mode>(pos: usize, state: State<M>) -> bool {
        T::fail_at(pos, state)
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T> Print for Raw<T> {
    fn print_to(&self, buf: &mut String) {
        buf.push_str(&self.0);
    }
}

/// The text `T::default()` prints, so a [`StringOf`](crate::StringOf)
/// defaults to empty.
impl<T: Default + Print> Default for Raw<T> {
    fn default() -> Self {
        Raw::from(T::default().print())
    }
}

// Not derived: only the text matters, so no bounds on `T`.

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
///
/// ```
/// # use tygr::*;
/// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
/// #[derive(Grammar)]
/// struct Assign(StringEq!("x="), Ranged<StringOf1<Digit>>);
/// let v = Assign::parse("x=42").unwrap();
/// assert_eq!((v.1.range(), v.1.as_str()), (2..4, "42"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ranged<T> {
    /// Start byte offset.
    pub start: usize,
    /// The ranged value.
    pub ranged: T,
    /// End byte offset (exclusive).
    pub end: usize,
}

impl<T> Ranged<T> {
    /// Construct from an already-known range.
    pub fn new(start: usize, ranged: T, end: usize) -> Self {
        Ranged { start, ranged, end }
    }

    /// The `start..end` byte range.
    pub fn range(&self) -> Range<usize> {
        self.start..self.end
    }

    /// Apply `f` to the ranged value, keeping the same range.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Ranged<U> {
        Ranged::new(self.start, f(self.ranged), self.end)
    }

    /// Borrow the ranged value, keeping the same range.
    pub fn as_ref(&self) -> Ranged<&T> {
        Ranged::new(self.start, &self.ranged, self.end)
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

impl<T> Deref for Ranged<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.ranged
    }
}

impl<T> IntoInner<T> for Ranged<T> {
    fn into_inner(self) -> T {
        self.ranged
    }
}

impl<T: Parse> Parse for Ranged<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let (ranged, end) = T::parse_at::<M>(input, pos, state)?;
        Some((M::map(ranged, |ranged| Ranged::new(pos, ranged, end)), end))
    }

    fn fail_at<M: Mode>(pos: usize, state: State<M>) -> bool {
        T::fail_at(pos, state)
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Ranged<T> {
    fn print_to(&self, buf: &mut String) {
        self.ranged.print_to(buf);
    }
}

impl<T: Parse> Parse for Box<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let (value, end) = T::parse_at::<M>(input, pos, state)?;
        Some((M::map(value, Box::new), end))
    }

    fn fail_at<M: Mode>(pos: usize, state: State<M>) -> bool {
        T::fail_at(pos, state)
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Box<T> {
    fn print_to(&self, buf: &mut String) {
        (**self).print_to(buf);
    }
}

/// Matches `T` and keeps nothing, so it doesn't print: derive `Parse` alone,
/// or give the field a `print_to`.
impl<T: Parse> Parse for PhantomData<T> {
    type First = T::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let end = T::check_at(input, pos, state)?;
        Some((M::build(|_| PhantomData), end))
    }

    fn fail_at<M: Mode>(pos: usize, state: State<M>) -> bool {
        T::fail_at(pos, state)
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

/// The text of `input[pos..end]`; a grammar that ended mid-char panics here.
#[inline(always)]
pub(crate) fn text(input: &str, pos: usize, end: usize) -> &str {
    input.get(pos..end).expect("a grammar ended mid-char")
}
