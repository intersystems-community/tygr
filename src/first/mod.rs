//! Type-level FIRST sets: which bytes (and whether empty input) a grammar
//! could start with. See [`Grammar::First`] for how to build one.

use crate::grammar::Grammar;
use std::marker::PhantomData;

pub mod byte_set;

pub use byte_set::ByteSet;

/// Requires one of `T`'s bytes (not nullable).
pub struct Required<T: ByteSet>(PhantomData<T>);

/// Starts with one of `T`'s bytes, or matches empty input (nullable).
pub struct Optional<T: ByteSet>(PhantomData<T>);

/// A grammar's possible starting bytes, and whether it's nullable.
///
/// Nullability is the type ([`Required`] or [`Optional`]), so `Seq` can
/// skip what follows a `Required`; that keeps recursive grammars finite.
pub trait First {
    /// `Self` followed by grammar `G` (looked at only if `Self` is optional).
    type Seq<G: Grammar>: First;
    /// `Self` or `X`.
    type Alt<X: First>: First;
    /// `Self`, or empty input.
    type Opt: First;
    /// `Self` plus bytes `B`, same kind.
    #[doc(hidden)]
    type MapUnion<B: ByteSet>: First;
    /// The starting bytes.
    #[doc(hidden)]
    const BYTES: byte_set::Type;
    /// Can it match empty input?
    #[doc(hidden)]
    const NIL: bool;
}

/// Can a grammar with FIRST set `F` match when the next byte is `next`
/// (`None` at end of input)?
pub const fn contains<F: First>(next: Option<u8>) -> bool {
    match next {
        Some(b) => F::NIL || F::BYTES.contains(b),
        None => F::NIL,
    }
}

impl<T: ByteSet> First for Required<T> {
    type Seq<G: Grammar> = Self;
    type Alt<X: First> = X::MapUnion<T>;
    type Opt = Optional<T>;
    type MapUnion<B: ByteSet> = Required<byte_set::Union<T, B>>;
    const BYTES: byte_set::Type = T::BYTE_SET;
    const NIL: bool = false;
}

impl<T: ByteSet> First for Optional<T> {
    type Seq<G: Grammar> = <G::First as First>::MapUnion<T>;
    type Alt<X: First> = <X::Opt as First>::MapUnion<T>;
    type Opt = Self;
    type MapUnion<B: ByteSet> = Optional<byte_set::Union<T, B>>;
    const BYTES: byte_set::Type = T::BYTE_SET;
    const NIL: bool = true;
}

#[cfg(test)]
mod tests {
    use super::byte_set::{One, Type};
    use super::*;

    #[test]
    fn first_ops() {
        type Ab = <Required<One<b'a'>> as First>::Alt<Required<One<b'b'>>>;
        assert_eq!(Ab::BYTES, Type::range(b'a', b'b'));
        const { assert!(!Ab::NIL) };
        type Opt = <Ab as First>::Opt;
        const { assert!(Opt::NIL) };
        type Both = <Opt as First>::Alt<Required<One<b'z'>>>;
        assert!(Both::NIL && Both::BYTES.contains(b'z') && Both::BYTES.contains(b'a'));
    }

    // These only compile if `Seq` never looks past a required element:
    // otherwise each `First` would depend on itself.
    #[derive(crate::Grammar)]
    struct Paren(crate::StringEq!("("), Box<Paren>, crate::StringEq!(")"));

    #[derive(crate::Grammar)]
    struct Signed(
        Option<crate::StringEq!("-")>,
        crate::StringEq!("("),
        Box<Signed>,
    );

    #[test]
    fn recursion_stays_lazy() {
        type P = <Paren as crate::Grammar>::First;
        assert!(contains::<P>(Some(b'(')) && !contains::<P>(Some(b')')));
        assert!(!contains::<P>(None));

        type S = <Signed as crate::Grammar>::First;
        assert!(contains::<S>(Some(b'-')) && contains::<S>(Some(b'(')));
        assert!(!contains::<S>(None));
    }
}
