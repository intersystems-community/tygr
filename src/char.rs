//! Character class support for grammars.
//!
//! - [`CharClass`] — trait for character predicates
//! - [`CharOf<M>`] — match one character
//! - [`StringOf<C>`] — match zero or more characters
//! - [`StringOf1<C>`] — match one or more characters
//! - [`StringEq<T>`] — match a literal token described by a type-level chain

use std::char;
use std::fmt;
use std::marker::PhantomData;

#[cfg(feature = "trace_one_node")]
use crate::Expectation;
use crate::bnf::Expr;
use crate::first::{AnyChar, CharFirst, CharFirstCI, EmptyFirst, First};
use crate::grammar::Grammar;
use crate::state::State;
use crate::{IntoInner, Raw};

/// Trait for character-class predicates.
///
/// The easiest way to define one is the [`char_class!`](crate::char_class)
/// macro:
///
/// ```
/// # use tygr::*;
/// char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// char_class!(IsLower, "lower", |ch| matches!(ch, 'a'..='z'));
/// ```
///
/// For advanced cases you can implement the trait manually on a zero-sized
/// struct:
///
/// ```
/// # use tygr::*;
/// struct IsDigit;
/// impl CharClass for IsDigit {
///     fn matches(ch: char) -> bool { ch.is_ascii_digit() }
///     fn name() -> &'static str { "digit" }
/// }
/// ```
pub trait CharClass: 'static {
    /// Does `ch` belong to this character class?
    fn matches(ch: char) -> bool;

    /// Human-readable label (used in error messages and BNF).
    fn name() -> &'static str;
}

/// Matches exactly one character satisfying a character class.
pub struct CharOf<M: CharClass>(pub char, PhantomData<M>);

impl<M: CharClass> Clone for CharOf<M> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<M: CharClass> PartialEq for CharOf<M> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<M: CharClass> Eq for CharOf<M> {}

impl<M: CharClass> fmt::Debug for CharOf<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CharOf({:?})", self.0)
    }
}

impl<M: CharClass> CharOf<M> {
    /// Construct a `CharOf` from an already-known character.
    pub fn new(ch: char) -> Self {
        CharOf(ch, PhantomData)
    }

    /// The matched character.
    pub fn value(&self) -> char {
        self.0
    }
}

impl<M: CharClass> Grammar for CharOf<M> {
    type First = AnyChar;

    #[inline]
    fn parse_at(
        input: &str,
        pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> Option<(Self, usize)> {
        let ch = match input.as_bytes().get(pos) {
            // If ASCII, skips &str invariant checks
            Some(&b) if b < 0x80 => {
                if M::matches(b as char) {
                    Some((b as char, 1))
                } else {
                    None
                }
            }
            Some(_) => input[pos..]
                .chars()
                .next()
                .filter(|c: &char| M::matches(*c))
                .map(|c| (c, c.len_utf8())),
            None => None,
        };
        if let Some((ch, len)) = ch {
            Some((CharOf(ch, PhantomData), pos + len))
        } else {
            #[cfg(feature = "trace_pos")]
            state.expect(
                pos,
                #[cfg(feature = "trace_one_node")]
                Expectation::CharClass(M::name()),
            );
            None
        }
    }

    #[inline]
    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        Self::parse_at(input, pos, state).map(|(_, pos)| pos)
    }

    fn print_to(&self, buf: &mut String) {
        buf.push(self.0);
    }

    fn to_bnf() -> Expr {
        Expr::CharOf(M::name().to_string())
    }

    fn fail_at(
        #[allow(unused_variables)] pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> bool {
        #[cfg(feature = "trace_pos")]
        state.expect(
            pos,
            #[cfg(feature = "trace_one_node")]
            Expectation::CharClass(M::name()),
        );
        true
    }
}

/// Matches *zero or more* characters satisfying a character class, collected into a `String`.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// type Ws = StringOf<IsSpace>;  // optional whitespace
/// ```
pub struct StringOf<C: CharClass>(Raw<Vec<CharOf<C>>>);

impl<C: CharClass> Grammar for StringOf<C> {
    type First = <Raw<Vec<CharOf<C>>> as Grammar>::First;

    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let (inner, pos) = Raw::parse_at(input, pos, state)?;
        Some((StringOf(inner), pos))
    }

    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        Raw::<Vec<CharOf<C>>>::scan_at(input, pos, state)
    }

    fn print_to(&self, buf: &mut String) {
        self.0.print_to(buf);
    }

    fn to_bnf() -> Expr {
        Raw::<Vec<CharOf<C>>>::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        Raw::<Vec<CharOf<C>>>::fail_at(pos, state)
    }
}

/// Matches *one or more* characters satisfying a character class, collected into a `String`.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// #[derive(Grammar)]
/// struct Number(StringOf1<IsDigit>);  // one or more digits
/// ```
pub struct StringOf1<C: CharClass>(Raw<(CharOf<C>, Vec<CharOf<C>>)>);

impl<C: CharClass> Grammar for StringOf1<C> {
    type First = <Raw<(CharOf<C>, Vec<CharOf<C>>)> as Grammar>::First;

    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let (inner, pos) = Raw::parse_at(input, pos, state)?;
        Some((StringOf1(inner), pos))
    }

    fn scan_at(input: &str, pos: usize, state: State) -> Option<usize> {
        Raw::<(CharOf<C>, Vec<CharOf<C>>)>::scan_at(input, pos, state)
    }

    fn print_to(&self, buf: &mut String) {
        self.0.print_to(buf);
    }

    fn to_bnf() -> Expr {
        Raw::<(CharOf<C>, Vec<CharOf<C>>)>::to_bnf()
    }

    fn fail_at(pos: usize, state: State) -> bool {
        Raw::<(CharOf<C>, Vec<CharOf<C>>)>::fail_at(pos, state)
    }
}

/// Delegates the string-facing trait impls to the inner `Raw`.
macro_rules! impl_string_wrapper {
    ($ty:ident) => {
        impl<C: CharClass> fmt::Debug for $ty<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.0, f)
            }
        }
        impl<C: CharClass> fmt::Display for $ty<C> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
        impl<C: CharClass> Clone for $ty<C> {
            fn clone(&self) -> Self {
                $ty(self.0.clone())
            }
        }
        impl<C: CharClass> PartialEq for $ty<C> {
            fn eq(&self, other: &Self) -> bool {
                self.0 == other.0
            }
        }
        impl<C: CharClass> Eq for $ty<C> {}
        impl<C: CharClass> std::hash::Hash for $ty<C> {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.0.hash(state);
            }
        }
        impl<C: CharClass> std::ops::Deref for $ty<C> {
            type Target = str;
            fn deref(&self) -> &str {
                &self.0
            }
        }
        impl<C: CharClass> AsRef<str> for $ty<C> {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
        impl<C: CharClass> IntoInner<String> for $ty<C> {
            fn into_inner(self) -> String {
                self.0.0
            }
        }
    };
}

impl_string_wrapper!(StringOf);
impl_string_wrapper!(StringOf1);

/// Describes a literal-token chain: enough to match (`scan_at`) and describe
/// (`add_expectation`) itself; no `Self` value is ever built.
// `pub`, not private: `StringEq<T>`'s `Grammar::First = T::First` would
// otherwise leak a private trait's associated type (E0446).
#[doc(hidden)]
pub trait Token: 'static {
    type First: First;

    fn scan_at(input: &str, pos: usize, state: State) -> Result<usize, String>;

    fn add_expectation(str: &mut String);

    fn expectation() -> String {
        let mut str = String::new();
        Self::add_expectation(&mut str);
        str
    }
}

impl Token for () {
    type First = EmptyFirst;

    fn scan_at(_input: &str, pos: usize, _state: State) -> Result<usize, String> {
        Ok(pos)
    }

    fn add_expectation(_: &mut String) {}
}

/// Matches one literal character, then continues to the rest of the chain.
/// `StringEq!("…")` expands to a chain of these.
pub struct CharThen<const CH: char, T>(PhantomData<T>);

impl<const CH: char, T: Token> Token for CharThen<CH, T> {
    type First = CharFirst<CH>;

    #[inline]
    fn scan_at(
        input: &str,
        pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> Result<usize, String> {
        if input[pos..].starts_with(CH) {
            T::scan_at(input, pos + CH.len_utf8(), state)
        } else {
            Err(Self::expectation())
        }
    }

    fn add_expectation(str: &mut String) {
        str.push(CH);
        T::add_expectation(str);
    }
}

/// Case-insensitive [`CharThen`]. `StringEqCI!("…")` expands to a chain of these.
pub struct CharCIThen<const CH: char, T>(PhantomData<T>);

impl<const CH: char, T: Token> Token for CharCIThen<CH, T> {
    type First = CharFirstCI<CH>;

    #[inline]
    fn scan_at(
        input: &str,
        pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> Result<usize, String> {
        match input[pos..].chars().next() {
            Some(c) if c.eq_ignore_ascii_case(&CH) => T::scan_at(input, pos + c.len_utf8(), state),
            _ => Err(Self::expectation()),
        }
    }

    fn add_expectation(str: &mut String) {
        str.push(CH);
        T::add_expectation(str);
    }
}

/// `Grammar` for a `StringEq!` chain. Case-sensitive, so the matched text is
/// always exactly `T::expectation()` — nothing needs to be stored per match.
pub struct StringEq<T>(PhantomData<T>);

impl<T: Token> Grammar for StringEq<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let end = Self::scan_at(input, pos, state)?;
        Some((StringEq(PhantomData), end))
    }

    #[inline]
    #[cfg_attr(not(feature = "trace_pos"), allow(clippy::manual_ok_err))]
    fn scan_at(input: &str, pos: usize, mut state: State) -> Option<usize> {
        match T::scan_at(input, pos, state.reborrow()) {
            Ok(pos) => Some(pos),
            #[allow(unused_variables)]
            Err(expectation) => {
                #[cfg(feature = "trace_pos")]
                state.expect(
                    pos,
                    #[cfg(feature = "trace_one_node")]
                    Expectation::StringEq(expectation, T::expectation()),
                );
                None
            }
        }
    }

    fn print_to(&self, buf: &mut String) {
        T::add_expectation(buf);
    }

    fn to_bnf() -> Expr {
        Expr::StringEq(T::expectation())
    }

    fn fail_at(
        #[allow(unused_variables)] pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> bool {
        #[cfg(feature = "trace_pos")]
        state.expect(
            pos,
            #[cfg(feature = "trace_one_node")]
            Expectation::StringEq(T::expectation(), T::expectation()),
        );
        true
    }
}

impl<T: Token> fmt::Debug for StringEq<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StringEq").field(&T::expectation()).finish()
    }
}

// Hand-written: derive would add a `T` bound, but `T` is a phantom marker
// that's never stored, so these hold unconditionally.
impl<T> Clone for StringEq<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for StringEq<T> {}

impl<T> Default for StringEq<T> {
    fn default() -> Self {
        StringEq(PhantomData)
    }
}

impl<T> PartialEq for StringEq<T> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<T> Eq for StringEq<T> {}

/// `Grammar` for a `StringEqCI!` chain. Case-insensitive, so the matched text
/// can differ from `T::expectation()`'s casing — it's captured once here
/// rather than per chain link.
pub struct StringEqCI<T>(String, PhantomData<T>);

impl<T: Token> Grammar for StringEqCI<T> {
    type First = T::First;

    #[inline]
    fn parse_at(input: &str, pos: usize, state: State) -> Option<(Self, usize)> {
        let end = Self::scan_at(input, pos, state)?;
        Some((StringEqCI(input[pos..end].to_string(), PhantomData), end))
    }

    #[inline]
    #[cfg_attr(not(feature = "trace_pos"), allow(clippy::manual_ok_err))]
    fn scan_at(input: &str, pos: usize, mut state: State) -> Option<usize> {
        match T::scan_at(input, pos, state.reborrow()) {
            Ok(pos) => Some(pos),
            #[allow(unused_variables)]
            Err(expectation) => {
                #[cfg(feature = "trace_pos")]
                state.expect(
                    pos,
                    #[cfg(feature = "trace_one_node")]
                    Expectation::StringEqCI(expectation, T::expectation()),
                );
                None
            }
        }
    }

    fn print_to(&self, buf: &mut String) {
        buf.push_str(&self.0);
    }

    fn to_bnf() -> Expr {
        Expr::StringEqCI(T::expectation())
    }

    fn fail_at(
        #[allow(unused_variables)] pos: usize,
        #[allow(unused_variables, unused_mut)] mut state: State,
    ) -> bool {
        #[cfg(feature = "trace_pos")]
        state.expect(
            pos,
            #[cfg(feature = "trace_one_node")]
            Expectation::StringEqCI(T::expectation(), T::expectation()),
        );
        true
    }
}

impl<T> fmt::Debug for StringEqCI<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("StringEqCI").field(&self.0).finish()
    }
}

impl<T> Clone for StringEqCI<T> {
    fn clone(&self) -> Self {
        StringEqCI(self.0.clone(), PhantomData)
    }
}

impl<T: Token> Default for StringEqCI<T> {
    fn default() -> Self {
        StringEqCI(T::expectation(), PhantomData)
    }
}

impl<T> PartialEq for StringEqCI<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for StringEqCI<T> {}
