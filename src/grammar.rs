//! The traits that describe a grammar: [`Parse`] (in a [`Mode`]), [`Print`],
//! both as [`Grammar`], and [`Validate`].

use crate::bnf::{self, Expr, Rule, Rules};
use crate::first::First;
use crate::state::{Frames, run};
use crate::{Error, Frame, State};
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

    /// Parse the entire `input` as `Self`, failing if any input is left unconsumed.
    fn parse(input: &str) -> Result<Self, Error> {
        run(
            |mut state| {
                let (value, end) = Self::parse_at::<Build>(input, 0, state.reborrow())?;
                state.at_end(input, end).then_some(value)
            },
            |state| rerun::<Self>(input, state),
        )
    }

    /// Like [`parse`](Self::parse), but may stop early; returns the end
    /// position too.
    fn parse_prefix(input: &str) -> Result<(Self, usize), Error> {
        run(
            |state| Self::parse_at::<Build>(input, 0, state),
            |state| {
                Self::parse_at(input, 0, state);
            },
        )
    }

    /// Like [`parse`](Self::parse), but only checks `input`.
    fn check(input: &str) -> Result<(), Error> {
        run(
            |mut state| {
                let ((), end) = Self::parse_at::<Check>(input, 0, state.reborrow())?;
                state.at_end(input, end).then_some(())
            },
            |state| rerun::<Self>(input, state),
        )
    }

    /// Like [`check`](Self::check), but may stop early; returns the end
    /// position.
    fn check_prefix(input: &str) -> Result<usize, Error> {
        run(
            |state| Some(Self::parse_at::<Check>(input, 0, state)?.1),
            |state| {
                Self::parse_at(input, 0, state);
            },
        )
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

    /// Parse `Self` at byte offset `pos`, returning its output in mode `M`
    /// (the value, or nothing when checking) and the end offset, a char
    /// boundary (see [`Parse`]).
    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        state: State<M>,
    ) -> Option<(M::Output<Self>, usize)>;

    /// Record what this grammar expected at `pos`, as `M` does, without
    /// parsing (e.g. when `First` already ruled it out). Returns whether it's
    /// required (not nullable), so a sequence can stop after it.
    fn fail_at<M: Mode>(pos: usize, state: State<M>) -> bool;

    /// Parse at `pos`, building the value whatever the mode, recording as
    /// `M` does: for a part whose value is needed, e.g. to convert it.
    #[inline(always)]
    fn build_at<M: Mode>(input: &str, pos: usize, state: State<M>) -> Option<(Self, usize)> {
        let (value, end) = Self::parse_at(input, pos, state.built())?;
        Some((M::value(value), end))
    }

    /// Parse at `pos` for the end position alone, recording as `M` does.
    #[inline(always)]
    fn check_at<M: Mode>(input: &str, pos: usize, state: State<M>) -> Option<usize> {
        Some(Self::parse_at(input, pos, state.checked())?.1)
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

/// The second pass of a failed `parse` or `check`: the same parse, recording.
/// It only checks, as no value is kept.
fn rerun<T: Parse>(input: &str, mut state: State<Record<Check>>) {
    if let Some((_, end)) = T::parse_at(input, 0, state.reborrow()) {
        state.at_end(input, end);
    }
}

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
pub trait Mode: Copy + 'static + private::Sealed {
    /// What parsing a `T` gives: the `T`, or `()`.
    type Output<T>;

    /// Whether what was expected is recorded: only in [`Record`], the second
    /// pass of a parse that failed.
    const RECORD: bool;

    /// This mode, building values: what [`build_at`](Parse::build_at) parses
    /// in; [`value`](Self::value) unwraps its output.
    type Built: Mode;

    /// This mode, keeping only the end position.
    type Checked: Mode;

    /// The `T` a [`Built`](Self::Built) output holds.
    fn value<T>(output: <Self::Built as Mode>::Output<T>) -> T;

    /// The rules entered so far, a list through the stack, when recording;
    /// otherwise `()`.
    #[doc(hidden)]
    type Context<'a>: Copy
        + Default
        + Into<<Self::Built as Mode>::Context<'a>>
        + Into<<Self::Checked as Mode>::Context<'a>>;

    #[doc(hidden)]
    fn reborrow<'s, 'a: 's>(context: Self::Context<'a>) -> Self::Context<'s>;

    #[doc(hidden)]
    fn enter<'s, 'a: 's>(parent: &'s Self::Context<'a>, frame: Frame) -> Self::Context<'s>;

    /// The rules entered, innermost first.
    #[doc(hidden)]
    fn frames<'s, 'a: 's>(
        context: &'s Self::Context<'a>,
    ) -> impl Iterator<Item = Frame> + Clone + 's;

    /// An output built by `f`. Only [`Build`] (or [`Record`] of it) calls it,
    /// passing itself to [`get`](Self::get) the outputs it combines.
    fn build<T>(f: impl FnOnce(Self) -> T) -> Self::Output<T>;

    /// `output` with `f` applied to its value.
    #[inline(always)]
    fn map<T, U>(output: Self::Output<T>, f: impl FnOnce(T) -> U) -> Self::Output<U> {
        Self::build(|mode| f(mode.get(output)))
    }

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
    const RECORD: bool = false;
    type Built = Build;
    type Checked = Check;
    type Context<'a> = ();

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(_context: ()) {}

    #[inline(always)]
    fn enter<'s, 'a: 's>(_parent: &'s (), _frame: Frame) {}

    #[inline(always)]
    fn frames<'s, 'a: 's>(_context: &'s ()) -> impl Iterator<Item = Frame> + Clone + 's {
        std::iter::empty()
    }

    #[inline(always)]
    fn value<T>(output: T) -> T {
        output
    }

    #[inline(always)]
    fn build<T>(f: impl FnOnce(Self) -> T) -> T {
        f(Build)
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
    const RECORD: bool = false;
    type Built = Build;
    type Checked = Check;
    type Context<'a> = ();

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(_context: ()) {}

    #[inline(always)]
    fn enter<'s, 'a: 's>(_parent: &'s (), _frame: Frame) {}

    #[inline(always)]
    fn frames<'s, 'a: 's>(_context: &'s ()) -> impl Iterator<Item = Frame> + Clone + 's {
        std::iter::empty()
    }

    #[inline(always)]
    fn value<T>(output: T) -> T {
        output
    }

    #[inline(always)]
    fn build<T>(_f: impl FnOnce(Self) -> T) {}

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

/// Mode `M`, also recording what was expected at the furthest position: the
/// second pass of a parse that failed.
#[derive(Debug, Clone, Copy)]
pub struct Record<M: Mode>(M);

impl<M: Mode> Mode for Record<M> {
    type Output<T> = M::Output<T>;
    const RECORD: bool = true;
    type Built = Record<M::Built>;
    type Checked = Record<M::Checked>;
    type Context<'a> = Option<Frames<'a>>;

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(context: Option<Frames<'a>>) -> Option<Frames<'s>> {
        context
    }

    #[inline(always)]
    fn enter<'s, 'a: 's>(parent: &'s Option<Frames<'a>>, frame: Frame) -> Option<Frames<'s>> {
        Some(Frames {
            frame,
            parent: parent.as_ref(),
        })
    }

    #[inline(always)]
    fn frames<'s, 'a: 's>(
        context: &'s Option<Frames<'a>>,
    ) -> impl Iterator<Item = Frame> + Clone + 's {
        std::iter::successors(context.as_ref(), |frames| frames.parent).map(|frames| frames.frame)
    }

    #[inline(always)]
    fn value<T>(output: <M::Built as Mode>::Output<T>) -> T {
        M::value(output)
    }

    #[inline(always)]
    fn build<T>(f: impl FnOnce(Self) -> T) -> M::Output<T> {
        M::build(|mode| f(Record(mode)))
    }

    #[inline(always)]
    fn building() -> Option<Self> {
        M::building().map(Record)
    }

    #[inline(always)]
    fn get<T>(self, output: M::Output<T>) -> T {
        self.0.get(output)
    }

    #[inline(always)]
    fn get_mut<T>(self, output: &mut M::Output<T>) -> &mut T {
        self.0.get_mut(output)
    }
}

mod private {
    /// Only [`Build`](super::Build), [`Check`](super::Check) and
    /// [`Record`](super::Record) are modes.
    pub trait Sealed {}
    impl Sealed for super::Build {}
    impl Sealed for super::Check {}
    impl<M: super::Mode> Sealed for super::Record<M> {}
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
