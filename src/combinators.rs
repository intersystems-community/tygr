//! Combinators that change what's matched: sequence, alternation, optional,
//! repetition and lookahead.

use crate::IntoInner;
use crate::State;
use crate::bnf::{Expr, Rules};
use crate::first::{First, Optional, byte_set};
use crate::grammar::{Parse, Print, Success, Unit};
use crate::state::{Level, probe};
use either::Either::{self, Left, Right};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use tygr_derive::Grammar;

// `()`: the empty sequence.
impl Parse for () {
    type First = Optional<byte_set::Empty>;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        _input: &str,
        pos: usize,
        _state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        Some((S::output(|| ()), pos))
    }

    fn to_bnf() -> Expr {
        Expr::empty()
    }

    fn visit_rules(_rules: &mut Rules) {}
}

impl Print for () {
    fn print_to(&self, _buf: &mut String) {}
}

macro_rules! seq_first {
    ($acc:ty;) => { $acc };
    ($acc:ty; $T:ident $(, $rest:ident)*) => {
        seq_first!(<$acc as First>::Seq<$T>; $($rest),*)
    };
}

/// The tuple of the outputs `$done` then `$T…`, each `and_then` reaching its
/// value only when building.
macro_rules! seq_output {
    ($S:ident; ($($done:ident),*);) => { $S::output(|| ($($done,)*)) };
    ($S:ident; ($($done:ident),*); $T:ident $(, $rest:ident)*) => {
        $S::and_then($T, |$T| seq_output!($S; ($($done,)* $T); $($rest),*))
    };
}

macro_rules! impl_tuple {
    ($($idx:tt $T:ident),+) => {
        impl<$($T: Parse),+> Parse for ($($T,)+) {
            type First = seq_first!(Optional<byte_set::Empty>; $($T),+);

            #[inline]
            #[allow(non_snake_case)]
            fn parse_at<S: Success, L: Level>(
                input: &str,
                pos: usize,
                mut state: State<L>,
            ) -> Option<(S::Output<Self>, usize)> {
                $( let ($T, pos) = <$T>::parse_at::<S, L>(input, pos, state.reborrow())?; )+
                Some((seq_output!(S; (); $($T),+), pos))
            }

            fn to_bnf() -> Expr {
                Expr::sequence(vec![$(<$T>::to_bnf()),+])
            }

            fn visit_rules(rules: &mut Rules) {
                $( <$T>::visit_rules(rules); )+
            }
        }

        impl<$($T: Print),+> Print for ($($T,)+) {
            fn print_to(&self, buf: &mut String) {
                $(self.$idx.print_to(buf);)+
            }
        }
    };
}

impl_tuple!(0 A, 1 B);
impl_tuple!(0 A, 1 B, 2 C);
impl_tuple!(0 A, 1 B, 2 C, 3 D);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J, 10 K);

/// Sequence of three grammars, [`Deref`]-ing to the middle one.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Grammar)]
#[tygr(inline)]
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

    /// Apply `f` to the wrapped value, keeping `before` and `after`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Wrap<L, U, R> {
        Wrap::new(self.before, f(self.wrapped), self.after)
    }
}

impl<L, T, E, R> Wrap<L, Result<T, E>, R> {
    /// Swap `Wrap<L, Result<T, E>, R>` for `Result<Wrap<L, T, R>, E>`.
    pub fn transpose(self) -> Result<Wrap<L, T, R>, E> {
        Ok(Wrap::new(self.before, self.wrapped?, self.after))
    }
}

impl<L, T, R> Deref for Wrap<L, T, R> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.wrapped
    }
}

impl<L, T, R> IntoInner<T> for Wrap<L, T, R> {
    fn into_inner(self) -> T {
        self.wrapped
    }
}

/// Sequence of two grammars, [`Deref`]-ing to the second.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Grammar)]
#[tygr(inline)]
pub struct Prefix<P, T> {
    /// The leading element.
    pub prefix: P,
    /// The prefixed value.
    pub prefixed: T,
}

impl<P, T> Prefix<P, T> {
    /// Construct a `Prefix` from its two parts.
    pub fn new(prefix: P, prefixed: T) -> Self {
        Self { prefix, prefixed }
    }

    /// Apply `f` to the prefixed value, keeping `prefix`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prefix<P, U> {
        Prefix::new(self.prefix, f(self.prefixed))
    }
}

impl<P, T, E> Prefix<P, Result<T, E>> {
    /// Swap `Prefix<P, Result<T, E>>` for `Result<Prefix<P, T>, E>`.
    pub fn transpose(self) -> Result<Prefix<P, T>, E> {
        Ok(Prefix::new(self.prefix, self.prefixed?))
    }
}

impl<P, T> Deref for Prefix<P, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.prefixed
    }
}

impl<P, T> IntoInner<T> for Prefix<P, T> {
    fn into_inner(self) -> T {
        self.prefixed
    }
}

/// Sequence of two grammars, [`Deref`]-ing to the first.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Grammar)]
#[tygr(inline)]
pub struct Suffix<T, S> {
    /// The suffixed value.
    pub suffixed: T,
    /// The trailing element.
    pub suffix: S,
}

impl<T, S> Suffix<T, S> {
    /// Construct a `Suffix` from its two parts.
    pub fn new(suffixed: T, suffix: S) -> Self {
        Self { suffixed, suffix }
    }

    /// Apply `f` to the suffixed value, keeping `suffix`.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Suffix<U, S> {
        Suffix::new(f(self.suffixed), self.suffix)
    }
}

impl<T, S, E> Suffix<Result<T, E>, S> {
    /// Swap `Suffix<Result<T, E>, S>` for `Result<Suffix<T, S>, E>`.
    pub fn transpose(self) -> Result<Suffix<T, S>, E> {
        Ok(Suffix::new(self.suffixed?, self.suffix))
    }
}

impl<T, S> Deref for Suffix<T, S> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.suffixed
    }
}

impl<T, S> IntoInner<T> for Suffix<T, S> {
    fn into_inner(self) -> T {
        self.suffixed
    }
}

impl<A: Parse, B: Parse> Parse for Either<A, B> {
    type First = <A::First as First>::Alt<B::First>;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        mut state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        if let Some((a, end)) = A::parse_at::<S, L>(input, pos, state.reborrow()) {
            return Some((S::and_then(a, |a| S::output(|| Left(a))), end));
        }
        let (b, end) = B::parse_at::<S, L>(input, pos, state)?;
        Some((S::and_then(b, |b| S::output(|| Right(b))), end))
    }

    fn to_bnf() -> Expr {
        Expr::alternation(vec![A::to_bnf(), B::to_bnf()])
    }

    fn visit_rules(rules: &mut Rules) {
        A::visit_rules(rules);
        B::visit_rules(rules);
    }
}

impl<A: Print, B: Print> Print for Either<A, B> {
    fn print_to(&self, buf: &mut String) {
        match self {
            Left(a) => a.print_to(buf),
            Right(b) => b.print_to(buf),
        }
    }
}

impl<T: Parse> Parse for Option<T> {
    type First = <T::First as First>::Opt;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        Some(match T::parse_at::<S, L>(input, pos, state) {
            Some((value, end)) => (S::and_then(value, |value| S::output(|| Some(value))), end),
            None => (S::output(|| None), pos),
        })
    }

    fn to_bnf() -> Expr {
        Expr::optional(T::to_bnf())
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Option<T> {
    fn print_to(&self, buf: &mut String) {
        if let Some(value) = self {
            value.print_to(buf);
        }
    }
}

impl<T: Parse> Parse for Vec<T> {
    type First = <T::First as First>::Opt;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        mut pos: usize,
        mut state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        const {
            assert!(
                !<T::First as First>::NULLABLE,
                "a repeated grammar must not match empty"
            )
        };
        let mut items = S::output(Vec::new);
        while let Some((item, end)) = T::parse_at::<S, L>(input, pos, state.reborrow())
            && end != pos
        {
            items = S::and_then(items, |mut items| {
                S::and_then(item, |item| {
                    items.push(item);
                    S::output(|| items)
                })
            });
            pos = end;
        }
        Some((items, pos))
    }

    fn to_bnf() -> Expr {
        Expr::repetition(T::to_bnf())
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Vec<T> {
    fn print_to(&self, buf: &mut String) {
        for item in self {
            item.print_to(buf);
        }
    }
}

/// Like `Vec<T>`, but one or more items: `T { T }`. Neither accepts a `T`
/// that can match empty, as repeating it would never progress.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Vec1<T>(Vec<T>);

impl<T> Deref for Vec1<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> IntoInner<Vec<T>> for Vec1<T> {
    fn into_inner(self) -> Vec<T> {
        self.0
    }
}

/// Fails on no items, giving them back.
impl<T> TryFrom<Vec<T>> for Vec1<T> {
    type Error = Vec<T>;

    fn try_from(items: Vec<T>) -> Result<Self, Vec<T>> {
        if items.is_empty() {
            Err(items)
        } else {
            Ok(Vec1(items))
        }
    }
}

impl<T: Parse> Parse for Vec1<T> {
    type First = T::First;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        let (items, end) = <Vec<T>>::parse_at::<S, L>(input, pos, state)?;
        (end != pos).then(|| (S::and_then(items, |items| S::output(|| Self(items))), end))
    }

    fn to_bnf() -> Expr {
        Expr::sequence(vec![T::to_bnf(), <Vec<T>>::to_bnf()])
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
    }
}

impl<T: Print> Print for Vec1<T> {
    fn print_to(&self, buf: &mut String) {
        self.0.print_to(buf);
    }
}

/// Negative lookahead: consumes nothing; matches only when `G` doesn't
/// match next.
///
/// ```
/// # use tygr::*;
/// // "/" that is not the start of a "//" line comment.
/// #[derive(Grammar)]
/// struct Div(StringEq!("/"), NotFollowedBy<StringEq!("/")>);
/// assert!(Div::parse::<Value, Max>("/").is_ok());
/// assert!(Div::parse::<Value, Max>("//").is_err());
/// ```
pub struct NotFollowedBy<G>(PhantomData<G>);

impl<G: Parse> Parse for NotFollowedBy<G> {
    type First = Optional<byte_set::Empty>;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        mut state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        // Probe, so only what `G` matched is recorded, not what it expected.
        match probe(|state| G::parse_at::<Unit, _>(input, pos, state)) {
            Some(((), end)) => {
                state.not_followed_by(G::to_bnf, pos..end);
                None
            }
            None => Some((S::output(|| NotFollowedBy(PhantomData)), pos)),
        }
    }

    fn to_bnf() -> Expr {
        Expr::not_followed_by(G::to_bnf())
    }

    fn visit_rules(rules: &mut Rules) {
        G::visit_rules(rules);
    }
}

/// Positive lookahead: consumes nothing; matches only when `G` matches next.
///
/// ```
/// # use tygr::*;
/// // A key that must be followed by ":", without consuming the ":".
/// #[derive(Grammar)]
/// struct Key(StringEq!("k"), FollowedBy<StringEq!(":")>);
/// assert!(Key::parse::<Value, Max>("k").is_err());
/// assert_eq!(Key::parse_prefix::<Value, Max>("k:").unwrap().1, 1);
/// ```
pub struct FollowedBy<G>(PhantomData<G>);

impl<G: Parse> Parse for FollowedBy<G> {
    // Nullable, as it consumes nothing: a recursion behind it is left recursion.
    type First = Optional<byte_set::Empty>;

    #[inline]
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        // Probe, so a match records nothing past `pos`; on failure, rerun `G`
        // to record what it expected.
        let matched = probe(|state| G::parse_at::<Unit, _>(input, pos, state));
        if matched.is_none() {
            G::parse_at::<Unit, L>(input, pos, state);
        }
        matched.map(|_| (S::output(|| FollowedBy(PhantomData)), pos))
    }

    // `!!G`: `Expr` has only negative lookahead.
    fn to_bnf() -> Expr {
        Expr::not_followed_by(Expr::not_followed_by(G::to_bnf()))
    }

    fn visit_rules(rules: &mut Rules) {
        G::visit_rules(rules);
    }
}

// Not derived: `G` is phantom, so no bounds on it.
macro_rules! impl_lookahead {
    ($Lookahead:ident) => {
        impl<G> Print for $Lookahead<G> {
            fn print_to(&self, _buf: &mut String) {}
        }

        impl<G> Default for $Lookahead<G> {
            fn default() -> Self {
                $Lookahead(PhantomData)
            }
        }

        impl<G> fmt::Debug for $Lookahead<G> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($Lookahead))
            }
        }

        impl<G> Clone for $Lookahead<G> {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl<G> Copy for $Lookahead<G> {}

        impl<G> PartialEq for $Lookahead<G> {
            fn eq(&self, _other: &Self) -> bool {
                true
            }
        }

        impl<G> Eq for $Lookahead<G> {}

        impl<G> std::hash::Hash for $Lookahead<G> {
            fn hash<H: std::hash::Hasher>(&self, _state: &mut H) {}
        }
    };
}

impl_lookahead!(NotFollowedBy);
impl_lookahead!(FollowedBy);
