//! The traits that describe a grammar: [`Grammar`], [`GrammarRule`],
//! [`GrammarFrom`] and [`Validate`]. Parsing is ordered choice with
//! backtracking; no left recursion.

use crate::first::First;
use crate::state::run;
use crate::{Error, State, bnf::Expr};

/// Parse, print, and describe (as BNF) a grammar element.
///
/// Implement this by hand only for leaf/wrapper types; for `struct`s and
/// `enum`s, `#[derive(Grammar)]` generates it (see the crate-level docs).
pub trait Grammar: Sized + 'static {
    /// The bytes (and whether empty input) this grammar can start with, used
    /// to skip alternatives that can't match. A manual impl delegates
    /// (`T::First`), combines ([`Seq`](crate::first::First::Seq) /
    /// [`Alt`](crate::first::First::Alt)), or names a set
    /// ([`Required`](crate::first::Required) /
    /// [`Optional`](crate::first::Optional)).
    ///
    /// ```
    /// # use tygr::first::*;
    /// type Digits = byte_set::Between<b'0', b'9'>;
    /// type Digit = Required<Digits>;
    /// assert!(contains::<Digit>(Some(b'7')) && !contains::<Digit>(None));
    ///
    /// type MaybeDigit = Optional<Digits>;
    /// assert!(contains::<MaybeDigit>(None));
    ///
    /// // A digit or 'a'.
    /// type DigitOrA = <Digit as First>::Alt<Required<byte_set::One<b'a'>>>;
    /// assert!(contains::<DigitOrA>(Some(b'a')) && !contains::<DigitOrA>(None));
    ///
    /// // An optional digit, then any char (`Seq` takes the grammar itself).
    /// type DigitThenChar = <MaybeDigit as First>::Seq<char>;
    /// assert!(contains::<DigitThenChar>(Some(b'x')) && !contains::<DigitThenChar>(None));
    /// ```
    type First: First;

    /// Parse the entire `input` as `Self`, failing if any input is left unconsumed.
    fn parse(input: &str) -> Result<Self, Error> {
        run(|state| {
            let (value, end) = Self::parse_at(input.as_bytes(), 0, state)?;
            (end == input.len()).then_some(value)
        })
    }

    /// Like [`parse`](Self::parse), but may stop early; returns the end
    /// position too.
    fn parse_prefix(input: &str) -> Result<(Self, usize), Error> {
        run(|state| Self::parse_at(input.as_bytes(), 0, state))
    }

    /// Like [`parse`](Self::parse), but only checks `input`.
    fn scan(input: &str) -> Result<(), Error> {
        run(|state| (Self::scan_at(input.as_bytes(), 0, state)? == input.len()).then_some(()))
    }

    /// Like [`scan`](Self::scan), but may stop early; returns the end
    /// position.
    fn scan_prefix(input: &str) -> Result<usize, Error> {
        run(|state| Self::scan_at(input.as_bytes(), 0, state))
    }

    /// Parse `Self` at byte offset `pos`, returning it and the end offset.
    ///
    /// Invariant, relied on by `unsafe` UTF-8 conversions: `input` is valid
    /// UTF-8 and every position is a char boundary. Public grammars only
    /// advance by whole chars; byte-level ones (`ByteEq`) are internal.
    fn parse_at(input: &[u8], pos: usize, state: State) -> Option<(Self, usize)>;

    /// Like [`parse_at`](Self::parse_at), but returns only the end.
    fn scan_at(input: &[u8], pos: usize, state: State) -> Option<usize>;

    /// Serialize `self` back to bytes, appending to `buf`. Without the
    /// `print` feature, derived impls panic here.
    #[cfg(feature = "print")]
    fn print_to(&self, buf: &mut Vec<u8>);
    /// Panics: tygr's `print` feature is off.
    #[cfg(not(feature = "print"))]
    fn print_to(&self, _buf: &mut Vec<u8>) {
        unimplemented!("tygr's `print` feature is off")
    }

    /// This grammar as an EBNF expression, as other rules refer to it.
    fn to_bnf() -> Expr;

    /// Record what this grammar expected at `pos`, without parsing (e.g.
    /// when `First` already ruled it out). Returns whether it's required
    /// (not nullable), so a sequence can stop reporting after it.
    fn fail_at(pos: usize, state: State) -> bool;

    /// Serialize `self` to a new byte vector; see [`print_to`](Self::print_to).
    fn print_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.print_to(&mut buf);
        buf
    }

    /// Serialize `self` to a new `String` (invalid UTF-8 becomes `U+FFFD`).
    fn print(&self) -> String {
        match String::from_utf8(self.print_bytes()) {
            Ok(text) => text,
            Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
        }
    }
}

/// A named [`Grammar`] with its own BNF rule (see
/// [`bnf_rules!`](crate::bnf_rules)).
pub trait GrammarRule: Grammar {
    /// The rule's name in BNF output; defaults to the type name.
    const NAME: &'static str;

    /// This rule's definition ([`to_bnf`](Grammar::to_bnf) is how others
    /// refer to it).
    fn to_bnf_def() -> Expr;

    /// Format this rule as a complete BNF line: `NAME = <definition> .`.
    fn bnf_rule() -> String {
        let mut s = String::new();
        s.push_str(Self::NAME);
        s.push_str(" = ");
        let expr = Self::to_bnf_def();
        expr.format(&mut s).unwrap();
        s.push_str(" .");
        for (i, text) in expr.side_conditions().into_iter().enumerate() {
            s.push_str(if i == 0 { "\n\n" } else { "\n" });
            s.push_str(&format!("^{}: {text}", i + 1));
        }
        s
    }
}

/// The source grammar of a type built by `GrammarFromStr`,
/// `GrammarFromStrInfallible`, `GrammarFromOther` or `GrammarTryFromOther`,
/// and how to print it back.
pub trait GrammarFrom {
    /// The grammar actually parsed.
    type Source: Grammar;

    /// Serialize `self` back to bytes (see [`Grammar::print_to`]); optional
    /// without the `print` feature.
    #[cfg(feature = "print")]
    fn print_to(&self, buf: &mut Vec<u8>);
    /// Panics: tygr's `print` feature is off.
    #[cfg(not(feature = "print"))]
    fn print_to(&self, _buf: &mut Vec<u8>) {
        unimplemented!("tygr's `print` feature is off")
    }
}

/// Post-parse validation for `#[grammar(validated)]` types.
///
/// Rejecting a value backtracks as if the grammar hadn't matched.
pub trait Validate {
    /// What a value must satisfy, shown in errors and BNF as "must …".
    const REQUIREMENT: &'static str;

    /// Check the just-parsed value; return `false` to reject it.
    fn validate(&self) -> bool;
}
