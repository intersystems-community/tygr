//! Operators that change what's matched: sequence, alternation, optional,
//! repetition and lookahead.

use crate::IntoInner;
use crate::State;
use crate::bnf::{Expr, Rules};
use crate::first::{First, Optional, byte_set};
use crate::grammar::{Check, Mode, Parse, Print};
use either::Either::{self, Left, Right};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;
use tygr_derive::Grammar;

// `()`: the empty sequence.
impl Parse for () {
    type First = Optional<byte_set::Empty>;

    #[inline]
    fn parse_at<M: Mode>(
        _input: &str,
        pos: usize,
        _state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        Some((M::build(|_| ()), pos))
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        false
    }

    fn to_bnf() -> Expr {
        Expr::empty()
    }

    fn visit_rules(_rules: &mut Rules) {}
}

impl Print for () {
    fn print_to(&self, _buf: &mut String) {}
}

macro_rules! concat_first {
    ($acc:ty;) => { $acc };
    ($acc:ty; $T:ident $(, $rest:ident)*) => {
        concat_first!(<$acc as First>::Seq<$T>; $($rest),*)
    };
}

macro_rules! impl_tuple {
    ($($idx:tt $T:ident),+) => {
        impl<$($T: Parse),+> Parse for ($($T,)+) {
            type First = concat_first!(Optional<byte_set::Empty>; $($T),+);

            #[inline]
            fn parse_at<M: Mode>(
                input: &str,
                pos: usize,
                mut state: State,
            ) -> Option<(M::Output<Self>, usize)> {
                $(
                    #[allow(non_snake_case)]
                    let ($T, pos) = <$T>::parse_at::<M>(input, pos, state.reborrow())?;
                )+
                Some((M::build(|mode| ($(mode.get($T),)+)), pos))
            }

            fn fail_at(pos: usize, mut state: State) -> bool {
                $( <$T>::fail_at(pos, state.reborrow()) || )+ false
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
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        if let Some((a, end)) = A::parse_at::<M>(input, pos, state.reborrow()) {
            return Some((M::map(a, Left), end));
        }
        let (b, end) = B::parse_at::<M>(input, pos, state)?;
        Some((M::map(b, Right), end))
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
        let a = A::fail_at(pos, state.reborrow());
        let b = B::fail_at(pos, state);
        a && b
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
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        Some(match T::parse_at::<M>(input, pos, state) {
            Some((value, end)) => (M::map(value, Some), end),
            None => (M::build(|_| None), pos),
        })
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state);
        false
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
    fn parse_at<M: Mode>(
        input: &str,
        mut pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let mut items = M::build(|_| Vec::new());
        while let Some((item, end)) = T::parse_at::<M>(input, pos, state.reborrow())
            && end != pos
        {
            if let Some(mode) = M::building() {
                mode.get_mut(&mut items).push(mode.get(item));
            }
            pos = end;
        }
        Some((items, pos))
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state);
        false
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

/// Like `Vec<T>`, but one or more items: `T { T }`.
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

    // `Vec` stops at an item that matches empty, so an empty match is no items.
    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let (items, end) = <Vec<T>>::parse_at::<M>(input, pos, state)?;
        (end != pos).then(|| (M::map(items, Self), end))
    }

    fn fail_at(pos: usize, state: State) -> bool {
        T::fail_at(pos, state)
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

/// One or more items, with a separator between each pair: `T { S T }`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VecSep<T, S> {
    /// The `T` items, in order.
    pub items: Vec<T>,
    /// The `S` separators, in order — always exactly one fewer than `items`.
    pub seps: Vec<S>,
}

// By hand, so items and separators are pushed straight into their vectors.
impl<T: Parse, S: Parse> Parse for VecSep<T, S> {
    type First = <T::First as First>::Seq<Vec<(S, T)>>;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let (first, mut pos) = T::parse_at::<M>(input, pos, state.reborrow())?;
        let mut vec_sep = M::build(|_| VecSep {
            items: Vec::new(),
            seps: Vec::new(),
        });
        // `push`, not `vec![first]`, so the first allocation gets the
        // amortized capacity rather than exactly one.
        if let Some(mode) = M::building() {
            mode.get_mut(&mut vec_sep).items.push(mode.get(first));
        }
        while let Some((sep, after_sep)) = S::parse_at::<M>(input, pos, state.reborrow())
            && let Some((item, end)) = T::parse_at::<M>(input, after_sep, state.reborrow())
            && end != pos
        {
            if let Some(mode) = M::building() {
                let VecSep { items, seps } = mode.get_mut(&mut vec_sep);
                seps.push(mode.get(sep));
                items.push(mode.get(item));
            }
            pos = end;
        }
        Some((vec_sep, pos))
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
        T::fail_at(pos, state.reborrow()) || <Vec<(S, T)>>::fail_at(pos, state)
    }

    fn to_bnf() -> Expr {
        Expr::sequence(vec![T::to_bnf(), <Vec<(S, T)>>::to_bnf()])
    }

    fn visit_rules(rules: &mut Rules) {
        T::visit_rules(rules);
        S::visit_rules(rules);
    }
}

impl<T: Print, S: Print> Print for VecSep<T, S> {
    fn print_to(&self, buf: &mut String) {
        let mut items = self.items.iter();
        if let Some(first) = items.next() {
            first.print_to(buf);
        }
        for (sep, item) in self.seps.iter().zip(items) {
            sep.print_to(buf);
            item.print_to(buf);
        }
    }
}

impl<T, S> VecSep<T, S> {
    /// Each item paired with the separator after it (drops the last item).
    pub fn into_item_sep(self) -> impl Iterator<Item = (T, S)> {
        self.items.into_iter().zip(self.seps)
    }

    /// The first `T` item. Panics if `items` is empty.
    pub fn first(&self) -> &T {
        &self.items[0]
    }

    /// The last `T` item. Panics if `items` is empty.
    pub fn last(&self) -> &T {
        self.items.last().unwrap()
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
/// assert!(Div::parse("/").is_ok());
/// assert!(Div::parse("//").is_err());
/// ```
pub struct NotFollowedBy<G>(PhantomData<G>);

impl<G: Parse> Parse for NotFollowedBy<G> {
    type First = Optional<byte_set::Empty>;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        // Probe, so only what `G` matched is recorded, not what it expected.
        match state.probe(|state| G::parse_at::<Check>(input, pos, state)) {
            Some(((), end)) => {
                state.not_followed_by(G::to_bnf, pos..end);
                None
            }
            None => Some((M::build(|_| NotFollowedBy(PhantomData)), pos)),
        }
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        false
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
/// assert!(Key::parse("k").is_err());
/// assert_eq!(Key::parse_prefix("k:").unwrap().1, 1);
/// ```
pub struct FollowedBy<G>(PhantomData<G>);

impl<G: Parse> Parse for FollowedBy<G> {
    type First = G::First;

    #[inline]
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        G::parse_at::<Check>(input, pos, state)?;
        Some((M::build(|_| FollowedBy(PhantomData)), pos))
    }

    fn fail_at(pos: usize, state: State) -> bool {
        G::fail_at(pos, state)
    }

    // `!!G`: Wirth's EBNF has no positive lookahead.
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
