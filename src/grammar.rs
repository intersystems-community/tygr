//! The traits that describe a grammar: [`Parse`] (in a [`Mode`]), [`Print`],
//! both as [`Grammar`], and [`Validate`].

use crate::Error;
use crate::bnf::{self, Expr, Rule, Rules};
use crate::first::First;
use std::fmt::{self, Write};

/// Parse and describe (as EBNF) a grammar.
///
/// Implement this by hand only for leaf/wrapper types; for `struct`s and
/// `enum`s, `#[derive(Parse)]` or `#[derive(Grammar)]` generates it (see
/// the crate-level docs).
///
/// [`parse_at`](Self::parse_at) gets a char-boundary `pos` and must return a
/// char-boundary end, passing other grammars the same `input`, never a slice.
/// Mid-char, no `char` is found, and a [`Raw`](crate::Raw) or conversion
/// keeping the text panics.
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't parse as a grammar",
    note = "derive `Parse` for it, or convert the field or its type from a grammar `S` with `#[tygr(from_str = S)]` or `#[tygr(try_from = S)]`"
)]
pub trait Parse: Sized + 'static {
    /// The bytes (and whether empty input) this grammar can start with, used
    /// to skip alternatives that can't match: leaving one out skips a match,
    /// extra ones only cost attempts. A manual impl delegates
    /// (`T::First`), combines ([`Seq`](crate::first::First::Seq) /
    /// [`Alt`](crate::first::First::Alt) / [`Opt`](crate::first::First::Opt)),
    /// or names a set ([`Required`](crate::first::Required) /
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

    /// Like [`parse`](Self::parse), but may stop early; returns the end
    /// position too.
    fn parse<T: TraceLevel>(input: &str) -> Result<(Self, usize), Error> {
        let state = T::State::new();
        Self::parse_at::<Build, T::State>(input, 0, &mut state).ok_or_else(unimplemented!())
    }

    /// Parse `Self` at byte offset `pos`, returning its output in mode `M`
    /// (the value, or nothing when checking) and the end offset, a char
    /// boundary (see [`Parse`]).
    fn parse_at<M: Mode, S: StateTrait>(
        input: &str,
        pos: usize,
        state: &mut S,
    ) -> Option<(M::Output<Self>, usize)>;

    /// Record what this grammar expected at `pos`, as `M` does, without
    /// parsing (e.g. when `First` already ruled it out). Returns whether it's
    /// required (not nullable), so a sequence can stop after it.
    fn fail_at<S: StateTrait>(pos: usize, state: &mut S) -> bool;

    /// This grammar's rule (unless inline or hidden), then each rule it
    /// refers to, depth first, once per type: to sort or
    /// [rename](Rule::rename), then print.
    ///
    /// ```
    /// # use tygr::*;
    /// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
    /// #[derive(Parse)]
    /// struct Pair(Number, StringEq!(","), Number);
    /// #[derive(Parse)]
    /// struct Number(StringOf1<Digit>);
    ///
    /// let rules: Vec<String> = Pair::bnf_rules().iter().map(ToString::to_string).collect();
    /// assert_eq!(rules, ["Pair = Number \",\" Number .", "Number = 'digit' { 'digit' } ."]);
    /// ```
    fn bnf_rules() -> Vec<Rule> {
        bnf::rules::<Self>()
    }

    /// This grammar as an EBNF expression, as other rules refer to it.
    fn to_bnf() -> Expr;

    /// Visit the named rules [`to_bnf`](Self::to_bnf) refers to: a named
    /// grammar [inserts](Rules::insert) its own, then, if it's new, visits
    /// its parts'.
    fn visit_rules(rules: &mut Rules);
}

/// Print a grammar back as text.
///
/// Implemented for tygr's own grammars; for `struct`s and `enum`s,
/// `#[derive(Print)]` or `#[derive(Grammar)]` generates it. A converted field
/// or type prints with `#[tygr(print_to = f)]` instead, such as
/// [`print_display`].
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't print as a grammar",
    note = "derive `Print` for it, or print the field or its converted type with `#[tygr(print_to = f)]`, e.g. `tygr::print_display`"
)]
pub trait Print {
    /// Append `self`'s text to `buf`.
    fn print_to(&self, buf: &mut String);

    /// `self`'s text as a new `String`.
    fn print(&self) -> String {
        let mut buf = String::new();
        self.print_to(&mut buf);
        buf
    }
}

/// Print `value` with its `Display`, for `#[tygr(print_to = print_display)]`.
///
/// ```
/// # use tygr::*;
/// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
/// #[derive(Grammar)]
/// struct Port(#[tygr(from_str = StringOf1<Digit>, print_to = print_display)] u16);
/// assert_eq!(Port(8080).print(), "8080");
/// ```
pub fn print_display<T: fmt::Display>(value: &T, buf: &mut String) {
    write!(buf, "{value}").expect("a Display implementation returned an error");
}

/// A grammar that both parses and prints, as `#[derive(Grammar)]` makes.
pub trait Grammar: Parse + Print {}

impl<T: Parse + Print> Grammar for T {}

/// Whether [`Parse::parse_at`] builds values ([`Build`]) or only matches
/// ([`Check`]), so one function does both; [`Record`] wraps either to also
/// record what it expected.
///
/// ```
/// # use tygr::*;
/// # use tygr::bnf::{Expr, Rules};
/// // A `T` in parentheses, as a hand-written grammar: `map` builds the value
/// // only when building.
/// struct Paren<T>(T);
///
/// impl<T: Parse> Parse for Paren<T> {
///     type First = <StringEq!("(") as Parse>::First;
///
///     fn parse_at<M: Mode>(
///         input: &str,
///         pos: usize,
///         mut state: State<M>,
///     ) -> Option<(M::Output<Self>, usize)> {
///         let mut state = state.node("Paren", pos);
///         let pos = <StringEq!("(")>::check_at(input, pos, state.reborrow())?;
///         let (inner, pos) = T::parse_at(input, pos, state.reborrow())?;
///         let pos = <StringEq!(")")>::check_at(input, pos, state)?;
///         Some((M::map(inner, Paren), pos))
///     }
///
///     fn fail_at<M: Mode>(pos: usize, mut state: State<M>) -> bool {
///         <StringEq!("(")>::fail_at(pos, state.node("Paren", pos))
///     }
///
///     fn to_bnf() -> Expr {
///         Expr::RuleRef("Paren".to_string())
///     }
///
///     fn visit_rules(rules: &mut Rules) {
///         let def = || {
///             Expr::sequence(vec![
///                 <StringEq!("(")>::to_bnf(),
///                 T::to_bnf(),
///                 <StringEq!(")")>::to_bnf(),
///             ])
///         };
///         if rules.insert::<Self>("Paren", def) {
///             T::visit_rules(rules);
///         }
///     }
/// }
///
/// assert_eq!(Paren::<char>::parse("(x)").unwrap().0, 'x');
/// assert_eq!(Paren::<char>::bnf_rules()[0].to_string(), r#"Paren = "(" 'any' ")" ."#);
/// ```
pub trait Mode: Copy + 'static {
    /// What parsing a `T` gives: the `T`, or `()`.
    type Output<T>;

    /// Build new output
    fn map<T, U>(value: Self::Output<T>, f: impl FnOnce(T) -> Self::Output<U>) -> Self::Output<U>;

    /// output
    fn output<T>(value: T) -> Self::Output<T>;
}

/// Builds values.
#[derive(Debug, Clone, Copy)]
pub struct Build;

/// Only matches: there are no `Check` values, so nothing is built.
#[derive(Debug, Clone, Copy)]
pub struct Check;

impl Mode for Build {
    type Output<T> = T;

    fn map<T, U>(value: Self::Output<T>, f: impl FnOnce(T) -> U) -> Self::Output<U> {
        f(value)
    }

    fn output<T>(value: T) -> Self::Output<T> {
        value
    }
}

impl Mode for Check {
    type Output<T> = ();

    fn map<T, U>(_value: Self::Output<T>, _f: impl FnOnce(T) -> U) -> Self::Output<U> {
        ()
    }

    fn output<T>(_value: T) -> Self::Output<T> {
        ()
    }
}

pub trait TraceLevel {
    type State: StateTrait;
}

pub trait StateTrait {
    fn new() -> Self;
}

/// Post-parse validation for `#[tygr(validated)]` types.
///
/// Rejecting a value backtracks as if the grammar hadn't matched.
pub trait Validate {
    /// What a value must satisfy: "must …" in errors, a footnote in EBNF.
    const REQUIREMENT: &'static str;

    /// Check the just-parsed value; return `false` to reject it.
    fn validate(&self) -> bool;
}
