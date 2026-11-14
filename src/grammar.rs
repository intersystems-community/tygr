//! The traits that describe a grammar: [`Parse`] (yielding a [`Success`]), [`Print`],
//! both as [`Grammar`], and [`Validate`].

use crate::bnf::{self, Expr, Rule, Rules};
use crate::first::First;
use crate::{Error, Level, State};
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

    /// Parse the whole `input` as `Self`, yielding what `S` says ([`Value`] or
    /// [`Unit`]) and, on failure, an [`Error`] as traced at level `L`.
    fn parse<S: Success, L: Level>(input: &str) -> Result<S::Output<Self>, Error<L>> {
        L::run::<Self, S>(input, true).map(|(value, _)| value)
    }

    /// Like [`parse`](Self::parse), but may stop early; returns the end
    /// position too.
    fn parse_prefix<S: Success, L: Level>(
        input: &str,
    ) -> Result<(S::Output<Self>, usize), Error<L>> {
        L::run::<Self, S>(input, false)
    }

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

    /// Parse `Self` at byte offset `pos`, returning what `S` yields (the value,
    /// or `()`) and the end offset, a char boundary (see [`Parse`]); `state`
    /// traces as its [`Level`] does. A part whose value is needed whatever `S`,
    /// e.g. to convert it, is parsed with `parse_at::<Value, L>`; one whose
    /// value isn't, with `Unit`.
    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)>;

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

/// What [`Parse::parse_at`] yields: the [`Value`] parsed, or a [`Unit`] saying
/// it parsed, so one function does both. What the parse traces is the
/// [`State`]'s [`Level`], not its concern.
///
/// ```
/// # use tygr::*;
/// # use tygr::bnf::{Expr, Rules};
/// // A `T` in parentheses, as a hand-written grammar: the parentheses are
/// // only checked, and the value is built only when building.
/// struct Paren<T>(T);
///
/// impl<T: Parse> Parse for Paren<T> {
///     type First = <StringEq!("(") as Parse>::First;
///
///     fn parse_at<S: Success, L: Level>(
///         input: &str,
///         pos: usize,
///         mut state: State<L>,
///     ) -> Option<(S::Output<Self>, usize)> {
///         let mut state = state.node("Paren", pos);
///         let ((), pos) = <StringEq!("(")>::parse_at::<Unit, L>(input, pos, state.reborrow())?;
///         let (inner, pos) = T::parse_at::<S, L>(input, pos, state.reborrow())?;
///         let ((), pos) = <StringEq!(")")>::parse_at::<Unit, L>(input, pos, state)?;
///         Some((S::and_then(inner, |inner| S::output(|| Paren(inner))), pos))
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
/// assert_eq!(Paren::<char>::parse::<Value, Max>("(x)").unwrap().0, 'x');
/// assert_eq!(Paren::<char>::bnf_rules()[0].to_string(), r#"Paren = "(" 'any' ")" ."#);
/// ```
pub trait Success: 'static + private::Sealed {
    /// What parsing a `T` gives: the `T`, or `()`.
    type Output<T>;

    /// The output of `f`'s value, which only [`Value`] calls for: what it
    /// builds (a `String`, a conversion) costs nothing when checking.
    fn output<T>(f: impl FnOnce() -> T) -> Self::Output<T>;

    /// The output `f` makes of `output`'s value, which only [`Value`] has:
    /// to combine outputs, as a sequence does.
    fn and_then<T, U>(
        output: Self::Output<T>,
        f: impl FnOnce(T) -> Self::Output<U>,
    ) -> Self::Output<U>;
}

/// Yields the value parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Value;

/// Yields nothing but that it parsed: `()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unit;

impl Success for Value {
    type Output<T> = T;

    #[inline(always)]
    fn output<T>(f: impl FnOnce() -> T) -> T {
        f()
    }

    #[inline(always)]
    fn and_then<T, U>(output: T, f: impl FnOnce(T) -> U) -> U {
        f(output)
    }
}

impl Success for Unit {
    type Output<T> = ();

    #[inline(always)]
    fn output<T>(_f: impl FnOnce() -> T) {}

    #[inline(always)]
    fn and_then<T, U>((): (), _f: impl FnOnce(T)) {}
}

mod private {
    /// Only [`Value`](super::Value) and [`Unit`](super::Unit) are successes.
    pub trait Sealed {}
    impl Sealed for super::Value {}
    impl Sealed for super::Unit {}
}

/// Post-parse validation for `#[tygr(validated)]` types.
///
/// Rejecting a value backtracks as if the grammar hadn't matched.
pub trait Validate {
    /// What a value must satisfy: "must …" in errors, a footnote in EBNF.
    const REQUIREMENT: &'static str;

    /// Unit the just-parsed value; return `false` to reject it.
    fn validate(&self) -> bool;
}
