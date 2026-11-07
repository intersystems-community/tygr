//! Operators that change what's matched: sequence, alternation, optional,
//! repetition and lookahead.

use crate::IntoInner;
use crate::State;
use crate::bnf::Expr;
use crate::first::{First, Optional, byte_set};
use crate::grammar::{Grammar, GrammarRule};
use either::Either::*;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use tygr_derive::Grammar;

// `()`: the empty sequence.
impl Grammar for () {
    type First = Optional<byte_set::Emp>;

    #[inline]
    fn parse_at(_input: &[u8], pos: usize, _state: State) -> Option<(Self, usize)> {
        Some(((), pos))
    }

    #[inline]
    fn scan_at(_input: &[u8], pos: usize, _state: State) -> Option<usize> {
        Some(pos)
    }

    fn print_to(&self, _buf: &mut Vec<u8>) {}

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
        concat_first!(<$acc as First>::Seq<$T>; $($rest),*)
    };
}

macro_rules! impl_grammar_tuple {
    ($($idx:tt $T:ident),+) => {
        impl<$($T: Grammar),+> Grammar for ($($T,)+) {
            type First = concat_first!(Optional<byte_set::Emp>; $($T),+);

            #[inline]
            fn parse_at(
                input: &[u8],
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
                input: &[u8],
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



            fn print_to(&self, buf: &mut Vec<u8>) {
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

/// Sequence of three grammars, [`Deref`]-ing to the middle one.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Wrap<L, T, R> {
    /// The leading element.
    pub before: L,
    /// The wrapped value.
    pub wrapped: T,
    /// The trailing element.
    pub after: R,
}

impl<L, T, R> Wrap<L, T, R> {
    /// Construct a `Wrap` from its three parts.
    pub fn new(before: L, wrapped: T, after: R) -> Self {
        Self {
            before,
            wrapped,
            after,
        }
    }

    /// Apply `f` to the wrapped value, keeping `before`/`after` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Wrap<L, U, R> {
        Wrap {
            before: self.before,
            wrapped: f(self.wrapped),
            after: self.after,
        }
    }
}

impl<L, T, E, R> Wrap<L, Result<T, E>, R> {
    /// Swap `Wrap<L, Result<T, E>, R>` for `Result<Wrap<L, T, R>, E>`.
    pub fn transpose(self) -> Result<Wrap<L, T, R>, E> {
        Ok(Wrap {
            before: self.before,
            wrapped: self.wrapped?,
            after: self.after,
        })
    }
}

/// Sequence of two grammars, [`Deref`]-ing to the second.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Prefix<P, T> {
    prefix: P,
    /// The prefixed value.
    pub prefixed: T,
}

/// Sequence of two grammars, [`Deref`]-ing to the first.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Suffix<T, S> {
    suffixed: T,
    suffix: S,
}

impl<L, T, R> Deref for Wrap<L, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.wrapped
    }
}

impl<T, S> Deref for Suffix<T, S> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.suffixed
    }
}

impl<P, T> Deref for Prefix<P, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.prefixed
    }
}

impl<P, T> Prefix<P, T> {
    /// Construct a `Prefix` from its two parts.
    pub fn new(prefix: P, prefixed: T) -> Self {
        Self { prefix, prefixed }
    }

    /// Apply `f` to the prefixed value, keeping `prefix` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prefix<P, U> {
        Prefix {
            prefix: self.prefix,
            prefixed: f(self.prefixed),
        }
    }
}

impl<P, T, E> Prefix<P, Result<T, E>> {
    /// Swap `Prefix<P, Result<T, E>>` for `Result<Prefix<P, T>, E>`.
    pub fn transpose(self) -> Result<Prefix<P, T>, E> {
        Ok(Prefix {
            prefix: self.prefix,
            prefixed: self.prefixed?,
        })
    }
}

impl<T, S> Suffix<T, S> {
    /// Construct a `Suffix` from its two parts.
    pub fn new(suffixed: T, suffix: S) -> Self {
        Self { suffixed, suffix }
    }

    /// Apply `f` to the suffixed value, keeping `suffix` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Suffix<U, S> {
        Suffix {
            suffixed: f(self.suffixed),
            suffix: self.suffix,
        }
    }
}

impl<T, S, E> Suffix<Result<T, E>, S> {
    /// Swap `Suffix<Result<T, E>, S>` for `Result<Suffix<T, S>, E>`.
    pub fn transpose(self) -> Result<Suffix<T, S>, E> {
        Ok(Suffix {
            suffixed: self.suffixed?,
            suffix: self.suffix,
        })
    }
}

impl<L, T, R> IntoInner<T> for Wrap<L, T, R> {
    fn into_inner(self) -> T {
        self.wrapped
    }
}

impl<T, S> IntoInner<T> for Suffix<T, S> {
    fn into_inner(self) -> T {
        self.suffixed
    }
}

impl<P, T> IntoInner<T> for Prefix<P, T> {
    fn into_inner(self) -> T {
        self.prefixed
    }
}

impl<A: Grammar, B: Grammar> Grammar for either::Either<A, B> {
    type First = <A::First as First>::Alt<B::First>;

    #[inline]
    fn parse_at(input: &[u8], pos: usize, mut state: State) -> Option<(Self, usize)> {
        A::parse_at(input, pos, state.reborrow())
            .map(|(x, end_pos)| (Left(x), end_pos))
            .or_else(|| B::parse_at(input, pos, state).map(|(x, end_pos)| (Right(x), end_pos)))
    }

    #[inline]
    fn scan_at(input: &[u8], pos: usize, mut state: State) -> Option<usize> {
        A::scan_at(input, pos, state.reborrow()).or_else(|| B::scan_at(input, pos, state))
    }

    fn print_to(&self, buf: &mut Vec<u8>) {
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

impl<T: Grammar> Grammar for Option<T> {
    type First = <T::First as First>::Opt;

    #[inline]
    fn parse_at(input: &[u8], pos: usize, state: State) -> Option<(Self, usize)> {
        match T::parse_at(input, pos, state) {
            Some((val, new_pos)) => Some((Some(val), new_pos)),
            None => Some((None, pos)),
        }
    }

    #[inline]
    fn scan_at(input: &[u8], pos: usize, state: State) -> Option<usize> {
        match T::scan_at(input, pos, state) {
            Some(end_pos) => Some(end_pos),
            None => Some(pos),
        }
    }

    fn print_to(&self, buf: &mut Vec<u8>) {
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
    type First = <T::First as First>::Opt;

    #[inline]
    fn parse_at(input: &[u8], mut pos: usize, mut state: State) -> Option<(Self, usize)> {
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
    fn scan_at(input: &[u8], mut pos: usize, mut state: State) -> Option<usize> {
        while let Some(new_pos) = { T::scan_at(input, pos, state.reborrow()) } {
            if new_pos == pos {
                break;
            }
            pos = new_pos;
        }
        Some(pos)
    }

    fn print_to(&self, buf: &mut Vec<u8>) {
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

/// Like `Vec`, but matches *one or more* items rather than zero or more.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    fn parse_at(input: &[u8], pos: usize, state: State) -> Option<(Self, usize)> {
        let (x, pos) = <Vec<T>>::parse_at(input, pos, state).unwrap();
        if x.is_empty() {
            None
        } else {
            Some((Self(x), pos))
        }
    }

    #[inline]
    fn scan_at(input: &[u8], pos: usize, state: State) -> Option<usize> {
        let end = <Vec<T>>::scan_at(input, pos, state).unwrap();
        if end == pos { None } else { Some(end) }
    }

    fn print_to(&self, buf: &mut Vec<u8>) {
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

/// One or more items, with a separator between each pair: `T { S T }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VecSep<T, S> {
    /// The `T` items, in order.
    pub items: Vec<T>,
    /// The `S` separators, in order — always exactly one fewer than `items`.
    pub seps: Vec<S>,
}

// By hand, so items and separators are pushed straight into their vectors.
impl<T: Grammar, S: Grammar> Grammar for VecSep<T, S> {
    type First = <T::First as First>::Seq<Vec<(S, T)>>;

    #[inline]
    fn parse_at(input: &[u8], pos: usize, mut state: State) -> Option<(Self, usize)> {
        let (first, mut pos) = T::parse_at(input, pos, state.reborrow())?;
        // `push`, not `vec![first]`, so the first allocation gets the
        // amortized capacity rather than exactly one.
        let (mut items, mut seps) = (Vec::new(), Vec::new());
        items.push(first);
        while let Some((sep, after_sep)) = S::parse_at(input, pos, state.reborrow())
            && let Some((item, end)) = T::parse_at(input, after_sep, state.reborrow())
            && end != pos
        {
            seps.push(sep);
            items.push(item);
            pos = end;
        }
        Some((VecSep { items, seps }, pos))
    }

    #[inline]
    fn scan_at(input: &[u8], pos: usize, mut state: State) -> Option<usize> {
        let mut pos = T::scan_at(input, pos, state.reborrow())?;
        while let Some(after_sep) = S::scan_at(input, pos, state.reborrow())
            && let Some(end) = T::scan_at(input, after_sep, state.reborrow())
            && end != pos
        {
            pos = end;
        }
        Some(pos)
    }

    fn print_to(&self, buf: &mut Vec<u8>) {
        let mut items = self.items.iter();
        if let Some(first) = items.next() {
            first.print_to(buf);
        }
        for (sep, item) in self.seps.iter().zip(items) {
            sep.print_to(buf);
            item.print_to(buf);
        }
    }

    fn to_bnf() -> Expr {
        Expr::sequence(vec![T::to_bnf(), <Vec<(S, T)>>::to_bnf()])
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
        T::fail_at(pos, state.reborrow()) || <Vec<(S, T)>>::fail_at(pos, state)
    }
}

// As in 0.6, where it was derived with `#[grammar(inline)]`.
impl<T: Grammar, S: Grammar> GrammarRule for VecSep<T, S> {
    const NAME: &'static str = "VecSep";

    fn to_bnf_def() -> Expr {
        Self::to_bnf()
    }
}

impl<T, S> VecSep<T, S> {
    /// Each item paired with the separator after it (drops the last item).
    pub fn into_item_sep(self) -> impl Iterator<Item = (T, S)> {
        self.items.into_iter().zip(self.seps)
    }

    /// The first `T` item.
    pub fn first(&self) -> &T {
        &self.items[0]
    }

    /// The last `T` item.
    pub fn last(&self) -> &T {
        self.items.last().unwrap()
    }
}

/// Negative lookahead: matches empty input only when `G` doesn't match next.
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
    type First = Optional<byte_set::Emp>;

    #[inline]
    fn parse_at(input: &[u8], pos: usize, state: State) -> Option<(Self, usize)> {
        let pos = Self::scan_at(input, pos, state)?;
        Some((NotFollowedBy(PhantomData), pos))
    }

    #[inline]
    fn scan_at(input: &[u8], pos: usize, mut state: State) -> Option<usize> {
        // Probe so the lookahead doesn't touch the error trace.
        match state.probe(|state| G::scan_at(input, pos, state)) {
            Some(_) => None,
            None => Some(pos),
        }
    }

    fn print_to(&self, _buf: &mut Vec<u8>) {}

    fn to_bnf() -> Expr {
        Expr::NotFollowedBy(Box::new(G::to_bnf()))
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        false
    }
}

// Not derived: `G` is phantom, so no bounds on it.
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

/// Positive lookahead: matches empty input only when `G` matches next.
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
