//! Core [`Grammar`] trait and blanket implementations for standard Rust types.
//!
//! Every grammar type implements [`Grammar`], providing:
//! - **parsing**  — `parse_at(input, pos, State) → Option<(Self, usize)>`
//! - **printing** — `print_to(&self, buf)`
//! - **BNF**      — `to_bnf() -> Expr`
//!
//! Parser uses ordered choice with backtracking. No left recursion.
//!
//! See the crate-level `## Design` section for how Rust constructs map to
//! EBNF concepts.

use crate::first::{EmptyFirst, First};
#[cfg(feature = "trace_one_node")]
use crate::state::Context;
#[cfg(feature = "trace_pos")]
use crate::state::History;
use crate::state::make_error;
use crate::{Error, IntoInner, State, bnf::Expr};
use either::Either::Left;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use tygr_derive::Grammar;

/// Parse, print, and describe (as BNF) a grammar element.
///
/// Implement this by hand only for leaf/wrapper types; for `struct`s and
/// `enum`s, `#[derive(Grammar)]` generates it (see the crate-level docs).
pub trait Grammar: Sized + 'static {
    /// The set of bytes (and whether empty input is valid) this grammar
    /// could start with — used to fast-path-reject an alternative before
    /// actually trying to parse it. A manual `impl Grammar` picks its
    /// `First` one of three ways, in order of preference:
    ///
    /// 1. **Delegate** to a grammar this one wraps or forwards to:
    ///    `type First = T::First;` (e.g. [`Raw<T>`](Raw), `Box<T>`).
    /// 2. **Build from a known, finite set of characters**: start from
    ///    [`first::Never`](crate::first::Never) (matches no byte, not
    ///    nullable) and chain
    ///    [`WithChar<C>`](crate::first::First::WithChar) /
    ///    [`WithCharCI<C>`](crate::first::First::WithCharCI) for each
    ///    character; finish with
    ///    [`Nullable`](crate::first::First::Nullable) if the grammar can
    ///    also match empty input.
    /// 3. **Over-approximate with [`first::AnyChar`](crate::first::AnyChar)**
    ///    (matches any byte) when the starting characters can't be
    ///    enumerated at compile time — e.g. behind an arbitrary runtime
    ///    predicate. `AnyChar` by itself is *not* nullable; add
    ///    [`Nullable`](crate::first::First::Nullable) if the grammar can
    ///    also match empty input.
    ///
    /// ```
    /// # use tygr::*;
    /// # use tygr::first::*;
    /// // Case 2: matches 'a' or 'b', not nullable.
    /// type AOrB = <<Never as First>::WithChar<'a'> as First>::WithChar<'b'>;
    ///
    /// // Case 3, nullable: matches any single character, or none.
    /// type MaybeAnyChar = <AnyChar as First>::Nullable;
    ///
    /// fn assert_first<T: First>() {}
    /// assert_first::<AOrB>();
    /// assert_first::<MaybeAnyChar>();
    /// ```
    type First: First;

    /// Parse the entire `input` as `Self`, failing if any input is left unconsumed.
    fn parse(input: &str) -> Result<Self, Error> {
        #[cfg(feature = "trace_pos")]
        let mut history = History::new();
        #[cfg(feature = "trace_one_node")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "trace_pos")]
            &mut history,
            #[cfg(feature = "trace_one_node")]
            context,
        );
        if let Some((val, pos)) = Self::parse_at(input, 0, state)
            && pos == input.len()
        {
            Ok(val)
        } else {
            Err(make_error(
                #[cfg(feature = "trace_pos")]
                history,
            ))
        }
    }

    /// Like [`parse`](Self::parse), but doesn't require consuming all of
    /// `input` — returns the byte position just past the match, leaving any
    /// remaining input unexamined.
    fn parse_prefix(input: &str) -> Result<(Self, usize), Error> {
        #[cfg(feature = "trace_pos")]
        let mut history = History::new();
        #[cfg(feature = "trace_one_node")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "trace_pos")]
            &mut history,
            #[cfg(feature = "trace_one_node")]
            context,
        );
        if let Some((val, pos)) = Self::parse_at(input, 0, state) {
            Ok((val, pos))
        } else {
            Err(make_error(
                #[cfg(feature = "trace_pos")]
                history,
            ))
        }
    }

    /// Like [`parse`](Self::parse), but only checks that `input` is well-formed
    /// and discards the parsed value.
    fn scan(input: &str) -> Result<(), Error> {
        #[cfg(feature = "trace_pos")]
        let mut history = History::new();
        #[cfg(feature = "trace_one_node")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "trace_pos")]
            &mut history,
            #[cfg(feature = "trace_one_node")]
            context,
        );
        if let Some(pos) = Self::scan_at(input, 0, state)
            && pos == input.len()
        {
            Ok(())
        } else {
            Err(make_error(
                #[cfg(feature = "trace_pos")]
                history,
            ))
        }
    }

    /// Like [`scan`](Self::scan), but doesn't require consuming all of
    /// `input` — returns the byte position just past the match, leaving any
    /// remaining input unexamined.
    fn scan_prefix(input: &str) -> Result<usize, Error> {
        #[cfg(feature = "trace_pos")]
        let mut history = History::new();
        #[cfg(feature = "trace_one_node")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "trace_pos")]
            &mut history,
            #[cfg(feature = "trace_one_node")]
            context,
        );
        if let Some(pos) = Self::scan_at(input, 0, state) {
            Ok(pos)
        } else {
            Err(make_error(
                #[cfg(feature = "trace_pos")]
                history,
            ))
        }
    }

    /// Attempt to parse `Self` starting at `pos`, returning the value and the
    /// position just past it, or `None` on failure.
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)>;

    /// Like [`parse_at`](Self::parse_at), but only checks well-formedness and
    /// returns the end position.
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize>;

    /// Serialize `self` back to text, appending to `buf`.
    fn print_to(&self, buf: &mut String);

    /// Describe this grammar as a BNF/EBNF expression, for use in a
    /// referencing rule's own definition.
    fn to_bnf() -> Expr;

    /// Record what this grammar would have expected at `pos`, without
    /// attempting to actually parse — used when a caller already knows (e.g.
    /// via `First`) that this alternative cannot match here, but still wants
    /// it traced.
    ///
    /// Returns whether this grammar is *required* at `pos` (`Self::First`
    /// doesn't contain nil). Sequential composition (`A::fail_at(..) ||
    /// B::fail_at(..)`) can stop once a required element reports itself,
    /// since real parsing would never reach anything after it either;
    /// nullable elements return `false` so the chain keeps going.
    fn fail_at(pos: usize, state: State) -> bool;

    /// Serialize `self` to a new `String`; see [`print_to`](Self::print_to).
    fn print(&self) -> String {
        let mut buf = String::new();
        self.print_to(&mut buf);
        buf
    }
}

/// A [`Grammar`] with a name and top-level BNF definition, so it can appear as
/// its own rule (e.g. in [`bnf_rules!`](crate::bnf_rules)) rather than only
/// inline in some other rule's definition.
pub trait GrammarRule: Grammar {
    /// The rule's name in BNF output; defaults to the type name.
    const NAME: &'static str;

    /// This rule's own definition, as opposed to [`to_bnf`](Grammar::to_bnf),
    /// which is how *other* rules refer to it.
    fn to_bnf_def() -> Expr;

    /// Format this rule as a complete BNF line: `NAME = <definition> .`.
    fn bnf_rule() -> String {
        let mut s = String::new();
        s.push_str(Self::NAME);
        s.push_str(" = ");
        let expr = Self::to_bnf_def();
        expr.format(&mut s).unwrap();
        s.push_str(" .");
        s
    }
}

/// Wrapper that hides a grammar element from BNF output.
///
/// Parses and prints just like the wrapped grammar, but is omitted from BNF.
/// Useful for structural elements like whitespace.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// type Ws = Hidden<StringOf<IsSpace>>;
/// ```
///
/// Or use `#[grammar(hidden)]`:
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// #[grammar(hidden)]
/// struct Ws(StringOf<IsSpace>);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Grammar)]
#[grammar(hidden)]
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

// ── Raw<T>  →  parse via T, store only the matched string ───────────────────

/// Wrapper that parses using the wrapped grammar but keeps only the raw
/// matched text as a `String`.
///
/// This is useful for grammar elements where the *structure* matters for
/// parsing (e.g. `Ws` defined as `StringOf<IsSpace>`), but consumers
/// only need the matched text.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// #[grammar(name = "ws", hidden)]
/// struct Ws(StringOf<IsSpace>);
///
/// # #[derive(Grammar)]
/// # struct Term(StringOf1<IsDigit>);
/// # #[derive(Grammar)]
/// # struct AddOp(StringEq!("+"));
/// // Whitespace stored as string
/// #[derive(Grammar)]
/// #[grammar(name = "expr")]
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

impl<T: Grammar> Grammar for Raw<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let new_pos = T::scan_at(input, pos, state)?;
        Some((Raw(input[pos..new_pos].to_string(), PhantomData), new_pos))
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        let end = T::scan_at(input, pos, state)?;
        Some(end)
    }

    fn print_to(&self, buf: &mut String) {
        buf.push_str(&self.0);
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }
}

// Manual trait impls — only the String matters, no bounds on T.

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

impl<T> std::ops::Deref for Raw<T> {
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

impl<T: Grammar> Grammar for Option<T> {
    type First = <T::First as First>::Nullable;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        match T::parse_at(input, pos, state) {
            Some((val, new_pos)) => Some((Some(val), new_pos)),
            None => Some((None, pos)),
        }
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        match T::scan_at(input, pos, state) {
            Some(end_pos) => Some(end_pos),
            None => Some(pos),
        }
    }

    fn print_to(&self, buf: &mut String) {
        if let Some(val) = self {
            val.print_to(buf);
        }
    }

    fn to_bnf() -> Expr {
        Expr::optional(T::to_bnf())
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state);
        false
    }
}

impl<T: Grammar> Grammar for Vec<T> {
    type First = <T::First as First>::Nullable;

    #[inline]
    fn parse_at(input: &str, mut pos: usize, mut state: State) -> Option<(Self, usize)> {
        let mut items: Vec<T> = Vec::new();
        while let Some((val, new_pos)) = { T::parse_at(input, pos, state.reborrow()) } {
            if new_pos == pos {
                break;
            }
            items.push(val);
            pos = new_pos;
        }
        Some((items, pos))
    }

    #[inline]
    fn scan_at(input: &str, mut pos: usize, mut state: State) -> Option<usize> {
        while let Some(new_pos) = { T::scan_at(input, pos, state.reborrow()) } {
            if new_pos == pos {
                break;
            }
            pos = new_pos;
        }
        Some(pos)
    }

    fn print_to(&self, buf: &mut String) {
        for item in self {
            item.print_to(buf);
        }
    }

    fn to_bnf() -> Expr {
        Expr::repetition(T::to_bnf())
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state);
        false
    }
}

impl<T: Grammar> Grammar for Box<T> {
    type First = T::First;

    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let (val, new_pos) = T::parse_at(input, pos, state)?;
        Some((Box::new(val), new_pos))
    }

    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        T::scan_at(input, pos, state)
    }

    fn print_to(&self, buf: &mut String) {
        (**self).print_to(buf);
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }
}

impl Grammar for () {
    type First = EmptyFirst;

    #[inline]
    fn parse_at(_input: &str, pos: usize, _state: State) -> Option<(Self, usize)> {
        Some(((), pos))
    }

    #[inline]
    fn scan_at(_input: &str, pos: usize, _state: State) -> Option<usize> {
        Some(pos)
    }

    fn print_to(&self, _buf: &mut String) {}

    fn to_bnf() -> Expr {
        Expr::empty()
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        false
    }
}

macro_rules! concat_first {
    ($acc:ty;) => { $acc };
    ($acc:ty; $T:ident $(, $rest:ident)*) => {
        concat_first!(<$acc as First>::Concat<$T>; $($rest),*)
    };
}

macro_rules! impl_grammar_tuple {
    ($($idx:tt $T:ident),+) => {
        impl<$($T: Grammar),+> Grammar for ($($T,)+) {
            type First = concat_first!(EmptyFirst; $($T),+);

            #[inline]
            fn parse_at(
                input: &str,
                pos: usize,
                #[allow(unused_mut)] mut state: State,
            ) -> Option<(Self, usize)> {
                let i = 0;
                $(
                    #[allow(non_snake_case)]
                    let ($T, pos) = <$T>::parse_at(input, pos, state.reborrow())?;
                    #[allow(unused_variables)]
                    let i = i + 1;
                )+
                Some((($($T,)+), pos))
            }

            #[inline]
            fn scan_at(
                input: &str,
                pos: usize,
                #[allow(unused_mut)] mut state: State,
            ) -> Option<usize> {
                let i = 0;
                $(
                    #[allow(non_snake_case)]
                    let pos =<$T>::scan_at(input, pos, state.reborrow())?;
                    #[allow(unused_variables)]
                    let i = i + 1;
                )+
                Some(pos)
            }



            fn print_to(&self, buf: &mut String) {
                $(self.$idx.print_to(buf);)+
            }

            fn to_bnf() -> Expr {
                Expr::sequence(vec![$(<$T>::to_bnf()),+])
            }

            fn fail_at(pos: usize, #[allow(unused_mut)] mut state: State) -> bool {
                $( <$T>::fail_at(pos, state.reborrow()) || )+ false
            }
        }
    };
}

impl_grammar_tuple!(0 A, 1 B);
impl_grammar_tuple!(0 A, 1 B, 2 C);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J);
impl_grammar_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K);

use either::Either::*;

impl<A: Grammar, B: Grammar> Grammar for either::Either<A, B> {
    type First = <A::First as First>::Union<B::First>;

    #[inline]
    fn parse_at(input: &str, pos: usize, mut state: State) -> Option<(Self, usize)> {
        A::parse_at(input, pos, state.reborrow())
            .map(|(x, end_pos)| (Left(x), end_pos))
            .or_else(|| B::parse_at(input, pos, state).map(|(x, end_pos)| (Right(x), end_pos)))
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, mut state: State) -> Option<usize> {
        A::scan_at(input, pos, state.reborrow()).or_else(|| B::scan_at(input, pos, state))
    }

    fn print_to(&self, buf: &mut String) {
        match self {
            Left(a) => a.print_to(buf),
            Right(b) => b.print_to(buf),
        }
    }

    fn to_bnf() -> Expr {
        Expr::alternation(vec![A::to_bnf(), B::to_bnf()])
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
        let a = A::fail_at(pos, state.reborrow());
        let b = B::fail_at(pos, state);
        a && b
    }
}

/// Zero-width negative lookahead: matches the empty string, but only when the
/// following input does *not* match the wrapped grammar. Consumes nothing
/// and prints nothing.
///
/// ```
/// # use tygr::*;
/// // "/" that is not the start of a "//" line comment.
/// #[derive(Grammar)]
/// struct Div(StringEq!("/"), NotFollowedBy<StringEq!("/")>);
/// assert!(Div::parse("/").is_ok());
/// assert!(Div::parse("//").is_err());
/// ```
pub struct NotFollowedBy<G>(PhantomData<G>);

impl<G: Grammar> Grammar for NotFollowedBy<G> {
    type First = EmptyFirst;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let pos = Self::scan_at(input, pos, state)?;
        Some((NotFollowedBy(PhantomData), pos))
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, mut state: State) -> Option<usize> {
        // Silent lookahead: probe G on a throwaway history so a match (or miss)
        // doesn't pollute the real error trace.
        match state.probe(|state| G::scan_at(input, pos, state)) {
            Some(_) => None,
            None => Some(pos),
        }
    }

    fn print_to(&self, _buf: &mut String) {}

    fn to_bnf() -> Expr {
        Expr::NotFollowedBy(Box::new(G::to_bnf()))
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        false
    }
}

// Hand-written rather than derived: `derive` would bound each impl on `G` (e.g.
// `G: Clone`), but `G` is a phantom marker that's never stored, so these hold
// unconditionally.
impl<G> Default for NotFollowedBy<G> {
    fn default() -> Self {
        NotFollowedBy(PhantomData)
    }
}

impl<G> fmt::Debug for NotFollowedBy<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("NotFollowedBy")
    }
}

impl<G> Clone for NotFollowedBy<G> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<G> Copy for NotFollowedBy<G> {}

impl<G> PartialEq for NotFollowedBy<G> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<G> Eq for NotFollowedBy<G> {}

impl<G> std::hash::Hash for NotFollowedBy<G> {
    fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
}

/// Zero-width positive lookahead: matches the empty string, but only when the
/// following input *does* match the wrapped grammar. Consumes nothing and
/// prints nothing.
///
/// ```
/// # use tygr::*;
/// // A key that must be followed by ":", without consuming the ":".
/// #[derive(Grammar)]
/// struct Key(StringEq!("k"), FollowedBy<StringEq!(":")>);
/// assert!(Key::parse("k").is_err());
/// assert_eq!(Key::parse_prefix("k:").unwrap().1, 1);
/// ```
pub type FollowedBy<G> = NotFollowedBy<NotFollowedBy<G>>;

/// Wrapper that records the `[start, end)` input span its ranged value was parsed from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range<T> {
    /// Byte offset where the ranged value started matching.
    pub start: usize,
    ranged: T,
    /// Byte offset just past where the ranged value finished matching.
    pub end: usize,
}

impl<T> IntoInner<T> for Range<T> {
    fn into_inner(self) -> T {
        self.ranged
    }
}

impl<T> Deref for Range<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.ranged
    }
}

impl<T> Range<T> {
    /// Construct a `Range` from an already-known span.
    pub fn new(start: usize, ranged: T, end: usize) -> Self {
        Range { start, ranged, end }
    }

    /// Apply `f` to the ranged value, keeping the same span.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Range<U> {
        Range {
            start: self.start,
            ranged: f(self.ranged),
            end: self.end,
        }
    }

    /// Borrow the ranged value, keeping the same span.
    pub fn as_ref(&self) -> Range<&T> {
        Range {
            start: self.start,
            ranged: &self.ranged,
            end: self.end,
        }
    }
}

impl<T> Range<Option<T>> {
    /// Swap `Range<Option<T>>` for `Option<Range<T>>`.
    pub fn transpose(self) -> Option<Range<T>> {
        match self.ranged {
            Some(it) => Some(Range {
                start: self.start,
                ranged: it,
                end: self.end,
            }),
            None => None,
        }
    }
}

impl<T, E> Range<Result<T, E>> {
    /// Swap `Range<Result<T, E>>` for `Result<Range<T>, E>`.
    pub fn transpose(self) -> Result<Range<T>, E> {
        match self.ranged {
            Ok(it) => Ok(Range {
                start: self.start,
                ranged: it,
                end: self.end,
            }),
            Err(e) => Err(e),
        }
    }
}

impl<T: Grammar> Grammar for Range<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        if let Some((it, new_pos)) = T::parse_at(input, pos, state) {
            Some((
                Range {
                    start: pos,
                    ranged: it,
                    end: new_pos,
                },
                new_pos,
            ))
        } else {
            None
        }
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        T::scan_at(input, pos, state)
    }

    fn print_to(&self, buf: &mut String) {
        self.ranged.print_to(buf);
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }
}

/// Like `Vec`, but matches *one or more* items rather than zero or more.
pub struct Vec1<T>(Vec<T>);

impl<T> Deref for Vec1<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Grammar> Grammar for Vec1<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let (x, pos) = <Vec<T>>::parse_at(input, pos, state).unwrap();
        if x.is_empty() {
            None
        } else {
            Some((Self(x), pos))
        }
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        let end = <Vec<T>>::scan_at(input, pos, state).unwrap();
        if end == pos { None } else { Some(end) }
    }

    fn print_to(&self, buf: &mut String) {
        for t in self.iter() {
            t.print_to(buf);
        }
    }

    fn to_bnf() -> Expr {
        Expr::sequence(vec![T::to_bnf(), <Vec<T>>::to_bnf()])
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
        T::fail_at(pos, state.reborrow()) || <Vec<T>>::fail_at(pos, state)
    }
}

/// Consumes and discards input matching `T`, storing nothing. Prints nothing
/// back, since there's no value left to print.
impl<T: Grammar> Grammar for PhantomData<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let pos = Self::scan_at(input, pos, state)?;
        Some((PhantomData, pos))
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        T::scan_at(input, pos, state)
    }

    fn print_to(&self, _buf: &mut String) {}

    fn to_bnf() -> Expr {
        T::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
    }
}

/// Bridges a mapped type to its source grammar.
///
/// Every conversion derive (`GrammarFromStr`, `GrammarFromOther`,
/// `GrammarTryFromOther`) requires this: [`Source`](GrammarFrom::Source) is the
/// grammar to parse (BNF and `FIRST` fold into it), and
/// [`print_to`](GrammarFrom::print_to) serializes back, since the generated
/// `Grammar` impl builds `Self` from the source but can't print it.
/// Implementations typically reconstruct the source grammar and delegate, or
/// write the canonical text directly.
pub trait GrammarFrom {
    /// The grammar actually parsed; `Self` is built from it after the fact.
    type Source: Grammar;

    /// Serialize `self` back to text (see [`Grammar::print_to`]).
    fn print_to(&self, buf: &mut String);
}
