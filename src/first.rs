//! Type-level FIRST sets: which bytes (and whether empty input) a grammar
//! could start with. See [`Grammar::First`] for how to build one.

use crate::grammar::Grammar;
use std::marker::PhantomData;

/// A grammar's set of possible starting bytes, and whether it's nullable
/// (can also match empty input).
///
/// See [`Grammar::First`] for how a manual `impl Grammar` typically builds
/// one — usually by delegating to another type's `First`, or starting from
/// [`Never`] or [`AnyChar`] and chaining [`WithChar`](First::WithChar) /
/// [`WithCharCI`](First::WithCharCI) / [`Nullable`](First::Nullable).
pub trait First {
    #[doc(hidden)]
    type Concat<G: Grammar>: First;
    #[doc(hidden)]
    type Union<X: First>: First;
    #[doc(hidden)]
    type UnionByteSet<X: ByteSet>: First;
    /// This `First`, but also matching empty input.
    type Nullable: First;
    /// This `First`, plus the byte(s) of `C`.
    type WithChar<const C: char>: First;
    /// This `First`, plus the byte(s) of both cases of `C`.
    type WithCharCI<const C: char>: First;
    #[doc(hidden)]
    const CONTAINS_BYTE: [bool; 256];
    #[doc(hidden)]
    const CONTAINS_NIL: bool;
}

// Internal plumbing: a `First` guaranteed never to match empty input, so
// `UnionByteSet` can combine two byte sets without worrying about nullability.
#[doc(hidden)]
pub trait ByteSet: First {}

/// The [`First`] that never matches — no byte, not even empty input. The
/// starting point for building a `First` from a known, finite set of
/// characters.
pub struct Never;
impl ByteSet for Never {}
impl First for Never {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X;

    type UnionByteSet<X: ByteSet> = X;

    type Nullable = OptionalFirst<Self>;

    type WithChar<const D: char> = AddChar<Self, D>;

    type WithCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = [false; 256];

    const CONTAINS_NIL: bool = false;
}

/// The [`First`] that matches any single byte, but (by itself) isn't
/// nullable. The over-approximation to reach for when the starting
/// characters can't be enumerated at compile time (e.g. a runtime
/// predicate). Add [`Nullable`](First::Nullable) if the grammar can also
/// match empty input.
pub struct AnyChar;
impl ByteSet for AnyChar {}
impl First for AnyChar {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = Self;

    type Nullable = OptionalFirst<Self>;

    type WithChar<const D: char> = AddChar<Self, D>;

    type WithCharCI<const D: char> = AddCharCI<Self, D>;

    const CONTAINS_BYTE: [bool; 256] = [true; 256];

    const CONTAINS_NIL: bool = false;
}

#[doc(hidden)]
pub struct AddChar<B: ByteSet, const C: char>(PhantomData<B>);
impl<B: ByteSet, const C: char> ByteSet for AddChar<B, C> {}
impl<B: ByteSet, const C: char> First for AddChar<B, C> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type Nullable = OptionalFirst<Self>;

    type WithChar<const D: char> = AddChar<Self, D>;

    type WithCharCI<const D: char> = AddCharCI<Self, D>;

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

#[doc(hidden)]
pub struct AddCharCI<B: ByteSet, const C: char>(PhantomData<B>);
impl<B: ByteSet, const C: char> ByteSet for AddCharCI<B, C> {}
impl<B: ByteSet, const C: char> First for AddCharCI<B, C> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type Nullable = OptionalFirst<Self>;

    type WithChar<const D: char> = AddChar<Self, D>;

    type WithCharCI<const D: char> = AddCharCI<Self, D>;

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
#[doc(hidden)]
pub struct UnionSet<A: ByteSet, B: ByteSet>(PhantomData<(A, B)>);
impl<A: ByteSet, B: ByteSet> ByteSet for UnionSet<A, B> {}
impl<A: ByteSet, B: ByteSet> First for UnionSet<A, B> {
    type Concat<G: Grammar> = Self;

    type Union<X: First> = X::UnionByteSet<Self>;

    type UnionByteSet<X: ByteSet> = UnionSet<Self, X>;

    type Nullable = OptionalFirst<Self>;

    type WithChar<const D: char> = AddChar<Self, D>;

    type WithCharCI<const D: char> = AddCharCI<Self, D>;

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

pub(crate) type CharFirst<const C: char> = AddChar<Never, C>;
pub(crate) type CharFirstCI<const C: char> = AddCharCI<Never, C>;

#[doc(hidden)]
pub struct OptionalFirst<B: ByteSet>(PhantomData<B>);

impl<B: ByteSet> First for OptionalFirst<B> {
    type Concat<G: Grammar> = <B as First>::Union<G::First>;

    type Union<X: First> = <X::UnionByteSet<B> as First>::Nullable;

    type UnionByteSet<X: ByteSet> = <B::UnionByteSet<X> as First>::Nullable;

    type Nullable = Self;

    type WithChar<const D: char> = <B::WithChar<D> as First>::Nullable;

    type WithCharCI<const D: char> = <B::WithCharCI<D> as First>::Nullable;

    const CONTAINS_BYTE: [bool; 256] = B::CONTAINS_BYTE;

    const CONTAINS_NIL: bool = true;
}

pub(crate) type EmptyFirst = OptionalFirst<Never>;
