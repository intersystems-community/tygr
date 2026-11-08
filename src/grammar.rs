//! The traits that describe a grammar: [`Parse`] (in a [`Mode`]), [`Print`],
//! both as [`Grammar`], [`GrammarRule`] and [`Validate`]. Parsing is
//! ordered choice with backtracking; no left recursion.

use crate::first::First;
use crate::state::run;
use crate::{Error, State, bnf::Expr};

/// Parse and describe (as BNF) a grammar element.
///
/// Implement this by hand only for leaf/wrapper types; for `struct`s and
/// `enum`s, `#[derive(Parse)]` or `#[derive(Grammar)]` generates it (see
/// the crate-level docs).
pub trait Parse: Sized + 'static {
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
            let (value, end) = Self::parse_at::<Build>(input.as_bytes(), 0, state)?;
            (end == input.len()).then_some(value)
        })
    }

    /// Like [`parse`](Self::parse), but may stop early; returns the end
    /// position too.
    fn parse_prefix(input: &str) -> Result<(Self, usize), Error> {
        run(|state| Self::parse_at::<Build>(input.as_bytes(), 0, state))
    }

    /// Like [`parse`](Self::parse), but only checks `input`.
    fn check(input: &str) -> Result<(), Error> {
        run(|state| {
            let ((), end) = Self::parse_at::<Check>(input.as_bytes(), 0, state)?;
            (end == input.len()).then_some(())
        })
    }

    /// Like [`check`](Self::check), but may stop early; returns the end
    /// position.
    fn check_prefix(input: &str) -> Result<usize, Error> {
        run(|state| Some(Self::parse_at::<Check>(input.as_bytes(), 0, state)?.1))
    }

    /// Parse `Self` at byte offset `pos`, returning its output in mode `M`
    /// (the value, or nothing when checking) and the end offset.
    ///
    /// Invariant, relied on by `unsafe` UTF-8 conversions: `input` is valid
    /// UTF-8 and every position is a char boundary. Public grammars only
    /// advance by whole chars; byte-level ones (`ByteEq`) are internal.
    fn parse_at<M: Mode>(
        input: &[u8],
        pos: usize,
        state: State,
    ) -> Option<(M::Output<Self>, usize)>;

    /// This grammar as an EBNF expression, as other rules refer to it.
    fn to_bnf() -> Expr;

    /// Record what this grammar expected at `pos`, without parsing (e.g.
    /// when `First` already ruled it out). Returns whether it's required
    /// (not nullable), so a sequence can stop reporting after it.
    fn fail_at(pos: usize, state: State) -> bool;
}

/// Print a grammar element back as text.
///
/// Implemented for tygr's own grammars; for `struct`s and `enum`s,
/// `#[derive(Print)]` or `#[derive(Grammar)]` generates it.
pub trait Print {
    /// Append `self`'s text to `buf`.
    fn print_to(&self, buf: &mut Vec<u8>);

    /// `self`'s text as a new `String` (invalid UTF-8 becomes `U+FFFD`).
    fn print(&self) -> String {
        let mut buf = Vec::new();
        self.print_to(&mut buf);
        match String::from_utf8(buf) {
            Ok(text) => text,
            Err(err) => String::from_utf8_lossy(err.as_bytes()).into_owned(),
        }
    }
}

/// A grammar that both parses and prints, as `#[derive(Grammar)]` makes.
pub trait Grammar: Parse + Print {}

impl<T: Parse + Print> Grammar for T {}

/// A named [`Parse`] grammar with its own BNF rule (see
/// [`bnf_rules!`](crate::bnf_rules)).
pub trait GrammarRule: Parse {
    /// The rule's name in BNF output; defaults to the type name.
    const NAME: &'static str;

    /// This rule's definition ([`to_bnf`](Parse::to_bnf) is how others
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

/// Whether [`Parse::parse_at`] builds values ([`Build`]) or only matches
/// ([`Check`]), so one function does both.
///
/// ```
/// # use tygr::*;
/// // A `T` in parentheses: `map` builds the value only when building.
/// struct Paren<T>(T);
///
/// fn parse_paren<T: Parse, M: Mode>(
///     input: &[u8],
///     pos: usize,
///     mut state: State,
/// ) -> Option<(M::Output<Paren<T>>, usize)> {
///     let ((), pos) = <StringEq!("(")>::parse_at::<Check>(input, pos, state.reborrow())?;
///     let (inner, pos) = T::parse_at::<M>(input, pos, state.reborrow())?;
///     let ((), pos) = <StringEq!(")")>::parse_at::<Check>(input, pos, state)?;
///     Some((M::map(inner, Paren), pos))
/// }
/// ```
pub trait Mode: Copy + 'static + private::Sealed {
    /// What parsing a `T` gives: the `T`, or `()`.
    type Output<T>;

    /// An output built by `f`. Only [`Build`] calls it, passing itself to
    /// [`get`](Self::get) the outputs it combines.
    fn build<T>(f: impl FnOnce(Self) -> T) -> Self::Output<T>;

    /// `output` with `f` applied to its value.
    fn map<T, U>(output: Self::Output<T>, f: impl FnOnce(T) -> U) -> Self::Output<U>;

    /// The mode when building, `None` when checking: for updating an output
    /// in place, such as pushing to a `Vec` in a loop.
    fn building() -> Option<Self>;

    /// The value in `output`.
    fn get<T>(self, output: Self::Output<T>) -> T;

    /// The value in `output`, mutably.
    fn get_mut<T>(self, output: &mut Self::Output<T>) -> &mut T;
}

/// Builds values.
#[derive(Debug, Clone, Copy)]
pub struct Build;

/// Only matches: there are no `Check` values, so nothing is built.
#[derive(Debug, Clone, Copy)]
pub enum Check {}

impl Mode for Build {
    type Output<T> = T;

    #[inline(always)]
    fn build<T>(f: impl FnOnce(Self) -> T) -> T {
        f(Build)
    }

    #[inline(always)]
    fn map<T, U>(output: T, f: impl FnOnce(T) -> U) -> U {
        f(output)
    }

    #[inline(always)]
    fn building() -> Option<Self> {
        Some(Build)
    }

    #[inline(always)]
    fn get<T>(self, output: T) -> T {
        output
    }

    #[inline(always)]
    fn get_mut<T>(self, output: &mut T) -> &mut T {
        output
    }
}

impl Mode for Check {
    type Output<T> = ();

    #[inline(always)]
    fn build<T>(_f: impl FnOnce(Self) -> T) {}

    #[inline(always)]
    fn map<T, U>((): (), _f: impl FnOnce(T) -> U) {}

    #[inline(always)]
    fn building() -> Option<Self> {
        None
    }

    fn get<T>(self, (): ()) -> T {
        match self {}
    }

    fn get_mut<T>(self, _output: &mut ()) -> &mut T {
        match self {}
    }
}

mod private {
    /// Only [`Build`](super::Build) and [`Check`](super::Check) are modes.
    pub trait Sealed {}
    impl Sealed for super::Build {}
    impl Sealed for super::Check {}
}

/// Post-parse validation for `#[tygr(validated)]` types.
///
/// Rejecting a value backtracks as if the grammar hadn't matched.
pub trait Validate {
    /// What a value must satisfy, shown in errors and BNF as "must …".
    const REQUIREMENT: &'static str;

    /// Check the just-parsed value; return `false` to reject it.
    fn validate(&self) -> bool;
}
