//! Terminal grammars: literals (`StringEq!` / `StringEqCI!`), `char`,
//! [`CharClass`]es and [`StringOf`] / [`StringOf1`].

use either::Either;
use std::fmt;
use std::ops::Range;

#[cfg(feature = "trace_one_node")]
use crate::Expectation;
use crate::bnf::{Expr, Rules};
use crate::first::{ByteSet, Required, byte_set};
use crate::grammar::{Grammar, Mode, Parse, Print};
use crate::state::State;
use crate::{Raw, Vec1};

/// Matches one literal byte.
#[doc(hidden)]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ByteEq<const B: u8>;

// SAFETY: it can end mid-char, so only literal macros use it, for a whole
// char's bytes at a time.
unsafe impl<const B: u8> Parse for ByteEq<B> {
    type First = Required<byte_set::One<B>>;

    // Records nothing on failure: its `Token` reports the whole literal.
    #[inline(always)]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        _state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        (input.get(pos) == Some(&B)).then(|| (M::build(|_| ByteEq), pos + 1))
    }

    fn fail_at(_pos: usize, _state: State) -> bool {
        true
    }

    fn to_bnf() -> Expr {
        unreachable!("internal: literal wrappers provide BNF")
    }

    fn visit_rules(_rules: &mut Rules) {}
}

impl<const B: u8> Print for ByteEq<B> {
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.push(B);
    }
}

/// A literal's bytes, built at compile time (stable Rust can't concatenate
/// `&'static [u8]` constants generically).
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct LiteralBuf {
    bytes: [u8; LiteralBuf::CAP],
    len: usize,
}

impl LiteralBuf {
    const CAP: usize = 256;

    const fn one(b: u8) -> LiteralBuf {
        let mut bytes = [0; LiteralBuf::CAP];
        bytes[0] = b;
        LiteralBuf { bytes, len: 1 }
    }

    const fn concat(self, other: LiteralBuf) -> LiteralBuf {
        assert!(
            self.len + other.len <= LiteralBuf::CAP,
            "literal over 256 bytes"
        );
        let mut out = self;
        let mut i = 0;
        while i < other.len {
            out.bytes[out.len + i] = other.bytes[i];
            i += 1;
        }
        out.len += other.len;
        out
    }

    const fn as_slice(&'static self) -> &'static [u8] {
        self.bytes.split_at(self.len).0
    }
}

/// A byte grammar for a fixed literal.
#[doc(hidden)]
pub trait Literal: Grammar {
    /// Does any part match other cases (contain an `Either`)?
    const CASE_INSENSITIVE: bool;
    /// The literal's bytes.
    const BUF: LiteralBuf;
    /// `BUF`, kept as a `'static` value to slice.
    const BUF_REF: &'static LiteralBuf = &Self::BUF;
    /// The literal's bytes.
    const BYTES: &'static [u8] = Self::BUF_REF.as_slice();
    /// The most bytes any of its case variants matches.
    const LONGEST: usize;

    /// Where a match from `pos` ends, or else where it diverged.
    fn reach(input: &[u8], pos: usize) -> Result<usize, usize>;
}

impl<const B: u8> Literal for ByteEq<B> {
    const CASE_INSENSITIVE: bool = false;
    const BUF: LiteralBuf = LiteralBuf::one(B);
    const LONGEST: usize = 1;

    fn reach(input: &[u8], pos: usize) -> Result<usize, usize> {
        if input.get(pos) == Some(&B) {
            Ok(pos + 1)
        } else {
            Err(pos)
        }
    }
}

impl<A: Literal, B: Literal> Literal for (A, B) {
    const CASE_INSENSITIVE: bool = A::CASE_INSENSITIVE || B::CASE_INSENSITIVE;
    const BUF: LiteralBuf = A::BUF.concat(B::BUF);
    const LONGEST: usize = A::LONGEST + B::LONGEST;

    fn reach(input: &[u8], pos: usize) -> Result<usize, usize> {
        B::reach(input, A::reach(input, pos)?)
    }
}

/// The left side is the literal: `StringEqCI!` puts each char first.
impl<A: Literal, B: Literal> Literal for Either<A, B> {
    const CASE_INSENSITIVE: bool = true;
    const BUF: LiteralBuf = A::BUF;
    const LONGEST: usize = if A::LONGEST > B::LONGEST {
        A::LONGEST
    } else {
        B::LONGEST
    };

    fn reach(input: &[u8], pos: usize) -> Result<usize, usize> {
        A::reach(input, pos).or_else(|a| B::reach(input, pos).map_err(|b| a.max(b)))
    }
}

/// A `StringEq!` / `StringEqCI!` literal; keeps the matched `G`.
#[doc(hidden)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Token<G: Literal>(G);

// SAFETY: a literal is whole chars.
unsafe impl<G: Literal> Parse for Token<G> {
    type First = G::First;

    #[inline(always)]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let result = G::parse_at::<M>(input, pos, state.reborrow());
        #[cfg(feature = "trace_pos")]
        if result.is_none() {
            Self::expected(pos..diverged::<G>(input, pos), state);
        }
        result.map(|(matched, end)| (M::map(matched, Token), end))
    }

    fn fail_at(pos: usize, state: State) -> bool {
        Self::expected(pos..pos, state);
        true
    }

    fn to_bnf() -> Expr {
        Expr::Token {
            text: Self::text(),
            case_insensitive: G::CASE_INSENSITIVE,
        }
    }

    fn visit_rules(_rules: &mut Rules) {}
}

impl<G: Literal> Print for Token<G> {
    fn print_to(&self, buf: &mut Vec<u8>) {
        self.0.print_to(buf);
    }
}

impl<G: Literal> Token<G> {
    const BYTES: &[u8] = G::BYTES;

    fn text() -> String {
        String::from_utf8_lossy(Self::BYTES).into_owned()
    }

    /// Record the literal as expected where it diverged, having matched
    /// `range`, like a rejected validation.
    fn expected(
        #[allow(unused_variables)] range: Range<usize>,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) {
        #[cfg(feature = "trace_pos")]
        state.expect(
            range.end,
            #[cfg(feature = "trace_one_node")]
            Expectation::Token {
                bytes: Self::BYTES,
                range,
            },
        );
    }
}

/// Where literal `G` diverged from `input` at `pos`, backed out of any char
/// it matched only part of.
#[cfg(feature = "trace_pos")]
fn diverged<G: Literal>(input: &[u8], pos: usize) -> usize {
    // One byte long, it can only diverge at once: skip the rescan.
    if G::LONGEST == 1 {
        return pos;
    }
    let (Ok(mut end) | Err(mut end)) = G::reach(input, pos);
    while input.get(end).is_some_and(|&byte| byte & 0xC0 == 0x80) {
        end -= 1;
    }
    end
}

impl<G: Literal> Default for Token<G> {
    fn default() -> Self {
        Self::parse(&Self::text()).unwrap()
    }
}

impl<G: Literal> fmt::Debug for Token<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Token").field(&self.print()).finish()
    }
}

/// UTF-8 lead bytes.
#[doc(hidden)]
pub type NonAsciiStart = byte_set::Between<0xC2, 0xF4>;

/// Bytes that can start a char.
pub(crate) type CharStart = byte_set::Union<byte_set::Between<0x00, 0x7F>, NonAsciiStart>;

/// `char`'s name in errors and BNF.
const ANY_CHAR: &str = "any";

/// Any one char. Fails only at end of input.
// SAFETY: ends after a whole char.
unsafe impl Parse for char {
    type First = Required<CharStart>;

    #[inline(always)]
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        match decode_char(input, pos) {
            Some((ch, end)) => Some((M::build(|_| ch), end)),
            None => {
                Self::fail_at(pos, state);
                None
            }
        }
    }

    fn fail_at(
        #[allow(unused_variables)] pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> bool {
        #[cfg(feature = "trace_pos")]
        state.expect(
            pos,
            #[cfg(feature = "trace_one_node")]
            Expectation::CharClass(ANY_CHAR),
        );
        true
    }

    fn to_bnf() -> Expr {
        Expr::CharOf(ANY_CHAR.to_string())
    }

    fn visit_rules(_rules: &mut Rules) {}
}

impl Print for char {
    fn print_to(&self, buf: &mut Vec<u8>) {
        push_char(buf, *self);
    }
}

/// The char at `pos` and its end; `None` at end of input.
#[inline(always)]
fn decode_char(input: &[u8], pos: usize) -> Option<(char, usize)> {
    // SAFETY: see `Parse`'s safety section.
    let ch = unsafe { std::str::from_utf8_unchecked(input.get(pos..)?) }
        .chars()
        .next()?;
    Some((ch, pos + ch.len_utf8()))
}

fn push_char(buf: &mut Vec<u8>, ch: char) {
    buf.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
}

/// A character class, defined by [`char_class!`](crate::char_class): the
/// grammar for one char satisfying a predicate.
///
/// ```
/// # use tygr::*;
/// char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
///
/// #[derive(Grammar)]
/// struct Digit(IsDigit);
///
/// assert_eq!(Digit::parse("7").unwrap().0.value(), '7');
/// assert!(Digit::parse("x").is_err());
/// ```
pub trait CharClass: Grammar + ByteSet {
    /// Does `ch` belong to this class?
    fn matches(ch: char) -> bool;

    /// Its name in errors and BNF.
    fn name() -> &'static str;

    /// Wrap `ch`, unchecked.
    fn new(ch: char) -> Self;

    /// The matched char.
    fn value(&self) -> char;
}

/// Define a [`CharClass`] from a `const` predicate, so its first bytes are
/// known at compile time. The name (for errors and BNF) defaults to the
/// type name:
///
/// ```
/// # use tygr::*;
/// char_class!(Digit, |ch| ch.is_ascii_digit()); // 'Digit'
/// char_class!(pub IsHex, "hex", |ch| matches!(ch, '0'..='9' | 'a'..='f')); // 'hex'
///
/// #[derive(Grammar)]
/// struct Num(StringOf1<Digit>);
/// ```
///
/// For a predicate that can't be `const` (e.g. `char::is_alphabetic`),
/// validate a `char` instead:
///
/// ```
/// # use tygr::*;
/// #[derive(Grammar)]
/// #[tygr(validated)]
/// struct Letter(char);
///
/// impl Validate for Letter {
///     const REQUIREMENT: &'static str = "be a letter";
///     fn validate(&self) -> bool {
///         self.0.is_alphabetic()
///     }
/// }
///
/// assert!(Letter::parse("é").is_ok() && Letter::parse("1").is_err());
/// ```
#[macro_export]
macro_rules! char_class {
    ($vis:vis $class:ident, |$ch:ident| $pred:expr) => {
        $crate::char_class!($vis $class, stringify!($class), |$ch| $pred);
    };
    ($vis:vis $class:ident, $name:expr, |$ch:ident| $pred:expr) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        $vis struct $class(pub char);

        impl $crate::CharClass for $class {
            fn matches($ch: char) -> bool { $pred }
            fn name() -> &'static str { $name }
            fn new(ch: char) -> Self { $class(ch) }
            fn value(&self) -> char { self.0 }
        }

        // First bytes: the ASCII ones that match, and every UTF-8 lead byte.
        impl $crate::first::ByteSet for $class {
            const BYTE_SET: $crate::first::byte_set::Type = {
                const fn matches($ch: char) -> bool { $pred }
                let mut bytes =
                    <$crate::__private::NonAsciiStart as $crate::first::ByteSet>::BYTE_SET;
                let mut b = 0u8;
                while b < 0x80 {
                    if matches(b as char) {
                        bytes = bytes.with(b);
                    }
                    b += 1;
                }
                bytes
            };
        }

        // SAFETY: ends after a whole char.
        unsafe impl $crate::Parse for $class {
            type First = $crate::first::Required<$class>;

            #[inline(always)]
            fn parse_at<M: $crate::Mode>(
                input: &[u8],
                pos: usize,
                state: $crate::State,
            ) -> Option<(M::Output<Self>, usize)> {
                $crate::__private::class_parse_at::<Self, M>(input, pos, state)
            }

            fn fail_at(pos: usize, state: $crate::State) -> bool {
                $crate::__private::class_fail_at::<Self>(pos, state)
            }

            fn to_bnf() -> $crate::bnf::Expr {
                $crate::__private::class_to_bnf::<Self>()
            }

            fn visit_rules(_rules: &mut $crate::bnf::Rules) {}
        }

        impl $crate::Print for $class {
            fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                $crate::__private::class_print_to(self, buf)
            }
        }
    };
}

/// `Parse::parse_at` for a `char_class!` type.
#[doc(hidden)]
#[inline(always)]
pub fn class_parse_at<C: CharClass, M: Mode>(
    input: &[u8],
    pos: usize,
    state: State,
) -> Option<(M::Output<C>, usize)> {
    let matched = match input.get(pos) {
        // The byte set is exact for ASCII: no decoding or predicate needed.
        Some(&b) if b.is_ascii() => C::BYTE_SET.contains(b).then_some((b as char, pos + 1)),
        _ => decode_char(input, pos).filter(|&(ch, _)| C::matches(ch)),
    };
    match matched {
        Some((ch, end)) => Some((M::build(|_| C::new(ch)), end)),
        None => {
            class_fail_at::<C>(pos, state);
            None
        }
    }
}

/// `Print::print_to` for a `char_class!` type.
#[doc(hidden)]
pub fn class_print_to<C: CharClass>(class: &C, buf: &mut Vec<u8>) {
    push_char(buf, class.value());
}

/// `Parse::to_bnf` for a `char_class!` type.
#[doc(hidden)]
pub fn class_to_bnf<C: CharClass>() -> Expr {
    Expr::CharOf(C::name().to_string())
}

/// `Parse::fail_at` for a `char_class!` type.
#[doc(hidden)]
pub fn class_fail_at<C: CharClass>(
    #[allow(unused_variables)] pos: usize,
    #[allow(unused_variables, unused_mut)] mut state: State,
) -> bool {
    #[cfg(feature = "trace_pos")]
    state.expect(
        pos,
        #[cfg(feature = "trace_one_node")]
        Expectation::CharClass(C::name()),
    );
    true
}

/// Zero or more chars of class `C`, kept as text.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// type Ws = StringOf<IsSpace>;
/// ```
pub type StringOf<C> = Raw<Vec<C>>;

/// One or more chars of class `C`, kept as text.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// #[derive(Grammar)]
/// struct Number(StringOf1<IsDigit>);
/// ```
pub type StringOf1<C> = Raw<Vec1<C>>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{StringEq, StringEqCI};

    type Bom = (ByteEq<0xEF>, (ByteEq<0xBB>, ByteEq<0xBF>));

    fn printed(grammar: &impl Print) -> Vec<u8> {
        let mut buf = Vec::new();
        grammar.print_to(&mut buf);
        buf
    }

    #[test]
    fn byte_eq_matches_and_prints_exact_bytes() {
        let bom = Bom::parse("\u{FEFF}").unwrap();
        assert_eq!(printed(&bom), b"\xEF\xBB\xBF");
        assert!(Bom::parse("x").is_err());
    }

    #[test]
    fn print_is_lossy_on_invalid_utf8() {
        let (lead, _) = ByteEq::<0xC3>::parse_prefix("é").unwrap();
        assert_eq!(printed(&lead), b"\xC3");
        assert_eq!(lead.print(), "\u{FFFD}");
    }

    #[test]
    fn literal_macros_expand_to_byte_atoms() {
        fn same<T>(_: T, _: T) {}
        same(
            <StringEq!("aé")>::default(),
            Token::<(ByteEq<b'a'>, (ByteEq<0xC3>, ByteEq<0xA9>))>::default(),
        );
        same(
            <StringEqCI!("a1")>::default(),
            Token::<(Either<ByteEq<b'a'>, ByteEq<b'A'>>, ByteEq<b'1'>)>::default(),
        );
    }
}
