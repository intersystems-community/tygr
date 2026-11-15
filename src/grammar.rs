//! The traits that describe a grammar: [`Parse`] (yielding a [`Success`]), [`Print`],
//! both as [`Grammar`], and [`Validate`].

use crate::bnf::{self, Expr, Rule, Rules};
use crate::first::First;
use crate::{Error, Level, State};
use success::{Unit, Value};

/// Parse and describe (as EBNF) a grammar.
///
/// Implement this by hand only for leaf/wrapper types; for `struct`s and
/// `enum`s, [`#[derive(Parse)]`](derive@crate::Parse) or
/// [`#[derive(Grammar)]`](derive@crate::Grammar) generates it (see the
/// crate-level docs).
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
    /// The bytes this grammar can start with, and whether it can match empty
    /// input. A dispatch table skips the alternatives whose `First` rules the
    /// next byte out. A byte left out, or empty input denied, skips a match;
    /// extra bytes only cost attempts.
    ///
    /// A hand-written grammar builds it from its parts', as the derive does:
    /// - the part it starts with: `<StringEq!("(") as Parse>::First`;
    /// - a sequence: the first part's, then
    ///   [`Seq`](crate::first::First::Seq) each later part, which counts only
    ///   while every part before it can match empty;
    /// - alternatives: [`Alt`](crate::first::First::Alt); an optional part:
    ///   [`Opt`](crate::first::First::Opt);
    /// - or a named set, [`Required`](crate::first::Required) or
    ///   [`Optional`](crate::first::Optional) of a
    ///   [`byte_set`](crate::first::byte_set): `Optional<byte_set::All>` is
    ///   always right, and only forgoes the skip.
    ///
    /// As `Seq` looks past a part only if it can match empty, a grammar may
    /// contain itself after a required part, but not start with itself (left
    /// recursion), which doesn't compile.
    ///
    /// ```
    /// # use tygr::*;
    /// # use tygr::first::*;
    /// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
    /// // What a part starts with.
    /// type Digits = <Vec1<Digit> as Parse>::First;
    /// assert!(contains::<Digits>(Some(b'7')) && !contains::<Digits>(None));
    ///
    /// // A sequence: `-`, or a digit, as the sign can be left out.
    /// type Signed = <<Option<StringEq!("-")> as Parse>::First as First>::Seq<Vec1<Digit>>;
    /// assert!(contains::<Signed>(Some(b'-')) && contains::<Signed>(Some(b'7')));
    /// assert!(!contains::<Signed>(None));
    ///
    /// // Alternatives, and an optional one.
    /// type Word = <Digits as First>::Alt<<StringEq!("x") as Parse>::First>;
    /// type MaybeWord = <Word as First>::Opt;
    /// assert!(contains::<Word>(Some(b'x')) && !contains::<Word>(None));
    /// assert!(contains::<MaybeWord>(None));
    ///
    /// // Named sets: a digit by its bytes, and anything at all.
    /// type ADigit = Required<byte_set::Between<b'0', b'9'>>;
    /// type Anything = Optional<byte_set::All>;
    /// assert!(contains::<ADigit>(Some(b'0')) && contains::<Anything>(None));
    /// ```
    type First: First;

    /// Parse the whole `input` as `Self`, yielding what `S` says ([`success::Value`] or
    /// [`success::Unit`]) and, on failure, an [`Error`] as traced at level `L`.
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
    /// e.g. to convert it, is parsed with `parse_at::<success::Value, L>`; one whose
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
/// [`#[derive(Print)]`](derive@crate::Print) or
/// [`#[derive(Grammar)]`](derive@crate::Grammar) generates it. A converted
/// field or type prints with `#[tygr(print_to = f)]` instead, an
/// `fn(&T, &mut String)` of yours: `Display` needn't invert the parse, so
/// tygr supplies none.
#[diagnostic::on_unimplemented(
    message = "`{Self}` doesn't print as a grammar",
    note = "derive `Print` for it, or print the field or its converted type with `#[tygr(print_to = f)]`, `f: fn(&T, &mut String)`"
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

/// A grammar that both parses and prints, as
/// [`#[derive(Grammar)]`](derive@crate::Grammar) makes.
pub trait Grammar: Parse + Print {}

impl<T: Parse + Print> Grammar for T {}

/// What [`Parse::parse_at`] yields: the [`success::Value`] parsed, or a [`success::Unit`] saying
/// it parsed, so one function does both. What the parse traces is the
/// [`State`]'s [`Level`], not its concern.
///
/// ```
/// # use tygr::*;
/// # use tygr::bnf::{Expr, Rules};
/// // A `T` in parentheses, as a hand-written grammar: the parentheses are
/// // parsed with `Unit`, and the value built only for `Value`.
/// struct Paren<T>(T);
///
/// impl<T: Parse> Parse for Paren<T> {
/// // Starts with `(`, whatever `T` is.
///     type First = <StringEq!("(") as Parse>::First;
///
///     fn parse_at<S: Success, L: Level>(
///         input: &str,
///         pos: usize,
///         mut state: State<L>,
///     ) -> Option<(S::Output<Self>, usize)> {
///         let mut state = state.node("Paren", pos);
///         let ((), pos) = <StringEq!("(")>::parse_at::<success::Unit, L>(input, pos, state.reborrow())?;
///         let (inner, pos) = T::parse_at::<S, L>(input, pos, state.reborrow())?;
///         let ((), pos) = <StringEq!(")")>::parse_at::<success::Unit, L>(input, pos, state)?;
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
/// assert_eq!(Paren::<char>::parse::<success::Value, level::Max>("(x)").unwrap().0, 'x');
/// assert_eq!(Paren::<char>::bnf_rules()[0].to_string(), r#"Paren = "(" 'any' ")" ."#);
/// ```
pub trait Success: 'static + private::Sealed {
    /// What parsing a `T` gives: the `T`, or `()`.
    type Output<T>;

    /// The output of `f`'s value, which only [`success::Value`] calls for: what it
    /// builds (a `String`, a conversion) costs nothing with [`success::Unit`].
    fn output<T>(f: impl FnOnce() -> T) -> Self::Output<T>;

    /// The output `f` makes of `output`'s value, which only [`success::Value`] has:
    /// to combine outputs, as a sequence does.
    fn and_then<T, U>(
        output: Self::Output<T>,
        f: impl FnOnce(T) -> Self::Output<U>,
    ) -> Self::Output<U>;
}

/// What a parse yields: the [`success::Value`] or only [`success::Unit`]. With `use tygr::*`, a
/// call reads `parse::<success::Value, level::Max>`.
pub mod success {
    /// The [`Success`](crate::Success) yielding the value parsed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Value;

    /// The [`Success`](crate::Success) yielding nothing but that it parsed:
    /// `()`.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Unit;
}

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
    /// Only [`success::Value`](super::success::Value) and [`success::Unit`](super::success::Unit) are successes.
    pub trait Sealed {}
    impl Sealed for super::success::Value {}
    impl Sealed for super::success::Unit {}
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
