//! Core [`Grammar`] trait and blanket implementations for standard Rust types.
//!
//! Every grammar type implements [`Grammar`], providing:
//! - **parsing**  — `parse_at(input, pos, State) → Option<(Self, usize)>`
//! - **printing** — `print_to(&self, buf)`
//! - **BNF**      — `write_bnf(w)`
//!
//! Parser uses ordered choice with backtracking. No left recursion.
//!
//! The library provides blanket impls so that standard Rust types map directly
//! to EBNF concepts:
//!
//! | Rust type       | EBNF concept     |
//! |-----------------|------------------|
//! | `struct`        | sequence (`A B`) |
//! | `enum`          | alternation (`A \| B`) |
//! | `(A, B, …)`     | inline sequence |
//! | `Either<A,B>`   | inline alternation |
//! | `Vec<T>`        | repetition (`T*`) |
//! | `Option<T>`     | optional (`[ T ]`) |
//! | `Box<T>`        | indirection (for recursive grammars) |
//! | `Hidden<T>`     | parsed & printed, but omitted from BNF |

use crate::char::{AnyChar, CharOf};
#[cfg(feature = "context")]
use crate::state::Context;
#[cfg(feature = "history")]
use crate::state::History;
use crate::state::make_error;
use crate::{Error, IntoInner, State, bnf::Expr};
use either::Either::Left;
#[cfg(feature = "trace")]
use std::any::TypeId;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use tygr_derive::Grammar;

pub trait First {
    type Concat<G: Grammar>: First;
    type Union<X: First>: First;
    type UnionByteSet<X: ByteSet>: First;
    type UEmpty: First;
    type UChar<const C: char>: First;
    type UCharCI<const C: char>: First;
    const CONTAINS_BYTE: [bool; 256];
    const CONTAINS_NIL: bool;
}

pub trait ByteSet: First {}

pub struct EmptyByteSet;
impl ByteSet for EmptyByteSet {}
impl First for EmptyByteSet {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X;

    type UnionByteSet<X: ByteSet> = X;

    type UEmpty = OptionalFirst<Self>;

    type UChar<const D: char> = AddChar<Self, D>;

    type UCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = [false; 256];

    const CONTAINS_NIL: bool = false;
}

pub struct AnyCharFirst;
impl ByteSet for AnyCharFirst {}
impl First for AnyCharFirst {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = Self;

    type UEmpty = OptionalFirst<Self>;

    type UChar<const D: char> = AddChar<Self, D>;

    type UCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = [true; 256];

    const CONTAINS_NIL: bool = false;
}

pub struct AddChar<B: ByteSet, const C: char>(PhantomData<B>);
impl<B: ByteSet, const C: char> ByteSet for AddChar<B, C> {}
impl<B: ByteSet, const C: char> First for AddChar<B, C> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type UEmpty = OptionalFirst<Self>;

    type UChar<const D: char> = AddChar<Self, D>;

    type UCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = {
        let mut map = B::CONTAINS_BYTE;
        map[first_byte(C) as usize] = true;
        map
    };

    const CONTAINS_NIL: bool = false;
}

/// First UTF-8 byte of `c` — the byte a first-set actually keys on.
const fn first_byte(c: char) -> u8 {
    let mut buf = [0u8; 4];
    c.encode_utf8(&mut buf);
    buf[0]
}

pub struct AddCharCI<B: ByteSet, const C: char>(PhantomData<B>);
impl<B: ByteSet, const C: char> ByteSet for AddCharCI<B, C> {}
impl<B: ByteSet, const C: char> First for AddCharCI<B, C> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type UEmpty = OptionalFirst<Self>;

    type UChar<const D: char> = AddChar<Self, D>;

    type UCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = {
        let mut map = B::CONTAINS_BYTE;
        map[first_byte(C.to_ascii_lowercase()) as usize] = true;
        map[first_byte(C.to_ascii_uppercase()) as usize] = true;
        map
    };

    const CONTAINS_NIL: bool = false;
}

/// Union of two byte sets — a single type node (O(1) depth per union) whose
/// byte map is the elementwise OR of its operands.
pub struct UnionSet<A: ByteSet, B: ByteSet>(PhantomData<(A, B)>);
impl<A: ByteSet, B: ByteSet> ByteSet for UnionSet<A, B> {}
impl<A: ByteSet, B: ByteSet> First for UnionSet<A, B> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type UEmpty = OptionalFirst<Self>;

    type UChar<const D: char> = AddChar<Self, D>;

    type UCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = {
        let a = A::CONTAINS_BYTE;
        let b = B::CONTAINS_BYTE;
        let mut map = [false; 256];
        let mut i = 0;
        while i < 256 {
            map[i] = a[i] || b[i];
            i += 1;
        }
        map
    };

    const CONTAINS_NIL: bool = false;
}

pub type CharFirst<const C: char> = AddChar<EmptyByteSet, C>;
pub type CharFirstCI<const C: char> = AddCharCI<EmptyByteSet, C>;

pub struct OptionalFirst<B: ByteSet>(PhantomData<B>);

impl<B: ByteSet> First for OptionalFirst<B> {
    type Concat<G: Grammar> = <B as First>::Union<G::First>;

    type Union<X: First> = <X::UnionByteSet<B> as First>::UEmpty;

    type UnionByteSet<X: ByteSet> = <B::UnionByteSet<X> as First>::UEmpty;

    type UEmpty = Self;

    type UChar<const D: char> = <B::UChar<D> as First>::UEmpty;

    type UCharCI<const D: char> = <B::UCharCI<D> as First>::UEmpty;

    const CONTAINS_BYTE: [bool; 256] = B::CONTAINS_BYTE;

    const CONTAINS_NIL: bool = true;
}

pub type EmptyFirst = OptionalFirst<EmptyByteSet>;
pub type AnyThingFirst = OptionalFirst<AnyCharFirst>;

pub trait Grammar: Sized + 'static {
    type First: First;

    #[cfg(feature = "trace")]
    fn type_id() -> TypeId {
        TypeId::of::<Self>()
    }

    fn parse(input: &str) -> Result<Self, Error> {
        #[cfg(feature = "history")]
        let mut history = History::new();
        #[cfg(feature = "context")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "history")]
            &mut history,
            #[cfg(feature = "context")]
            context,
        );
        if let Some((val, pos)) = Self::parse_at(input, 0, state)
            && pos == input.len()
        {
            Ok(val)
        } else {
            Err(make_error(
                #[cfg(feature = "history")]
                history,
            ))
        }
    }

    fn scan(input: &str) -> Result<(), Error> {
        #[cfg(feature = "history")]
        let mut history = History::new();
        #[cfg(feature = "context")]
        let context = Context::new();
        let state = State::new(
            #[cfg(feature = "history")]
            &mut history,
            #[cfg(feature = "context")]
            context,
        );
        if let Some(pos) = Self::scan_at(input, 0, state)
            && pos == input.len()
        {
            Ok(())
        } else {
            Err(make_error(
                #[cfg(feature = "history")]
                history,
            ))
        }
    }

    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)>;

    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize>;

    fn print_to(&self, buf: &mut String);

    fn to_bnf() -> Expr;

    fn print(&self) -> String {
        let mut buf = String::new();
        self.print_to(&mut buf);
        buf
    }
}

pub trait GrammarRule: Grammar {
    const NAME: &'static str;

    fn to_bnf_def() -> Expr;

    fn bnf_rule() -> String {
        let mut s = String::new();
        s.push_str(Self::NAME);
        s.push_str(" = ");
        let expr = Self::to_bnf_def();
        expr.format(&mut s).unwrap();
        s.push_str(" ;");
        s
    }
}

/// Wrapper that hides a grammar element from BNF output.
///
/// `Hidden<T>` parses and prints like `T`, but is omitted from BNF.
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

/// Wrapper that parses using `T`'s grammar but keeps only the raw matched
/// text as a `String`.
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
    type First = <T::First as First>::UEmpty;

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
}

impl<T: Grammar> Grammar for Vec<T> {
    type First = <T::First as First>::UEmpty;

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
        Expr::Empty
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
}

/// Consumes everything up to **and including** the next match of `T`, capturing
/// the matched text as a `String`. Fails if `T` never matches before end of input.
///
/// A newtype over the PEG idiom `( !T . )* T`: skip any character while `T` does
/// not match here, then consume `T`.
pub type StringEndsWith<T> = Raw<(Vec<(NotFollowedBy<T>, CharOf<AnyChar>)>, T)>;

/// Zero-width negative lookahead: matches the empty string, but only when the
/// following input does **not** match `G`. Consumes nothing and prints nothing.
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range<T> {
    pub start: usize,
    ranged: T,
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
    pub fn new(start: usize, ranged: T, end: usize) -> Self {
        Range { start, ranged, end }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Range<U> {
        Range {
            start: self.start,
            ranged: f(self.ranged),
            end: self.end,
        }
    }

    pub fn as_ref(&self) -> Range<&T> {
        Range {
            start: self.start,
            ranged: &self.ranged,
            end: self.end,
        }
    }
}

impl<T> Range<Option<T>> {
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
}

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
}

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

    fn print_to(&self, _buf: &mut String) {
        panic!("Can't print PhantomData")
    }

    fn to_bnf() -> Expr {
        T::to_bnf()
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
    type Source: Grammar;
    fn print_to(&self, buf: &mut String);
}
