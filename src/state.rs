//! The state threaded through parsing, and the [`Error`] it builds.

use crate::bnf::{Expr, Lazy, escape};
use crate::grammar::{Parse, Success};
use crate::success::Unit;
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

use level::{Max, Min, Pos};

/// Parse state threaded through [`Parse::parse_at`](crate::Parse::parse_at),
/// tracing at level `L`. A hand-written grammar enters its rule with
/// [`node`](Self::node), passes a [`reborrow`](Self::reborrow) to each
/// grammar it delegates to, and says what it [`expect`](Self::expect)ed
/// where it fails on its own.
pub struct State<'a, L: Level>(L::State<'a>);

impl<'a, L: Level> State<'a, L> {
    /// Reborrow for a nested call.
    #[inline(always)]
    pub fn reborrow(&mut self) -> State<'_, L> {
        State(L::reborrow(&mut self.0))
    }

    /// Enter rule `node` at `pos`, so that what's recorded inside names it.
    #[inline(always)]
    pub fn node(&mut self, node: &'static str, pos: usize) -> State<'_, L> {
        State(L::enter(&mut self.0, Frame { node, pos }))
    }

    /// Whether `pos` is the end of `input`; if not, records that it was
    /// expected.
    pub(crate) fn at_end(&mut self, input: &str, pos: usize) -> bool {
        let end = pos == input.len();
        if !end {
            self.expect(pos, || Expectation::End);
        }
        end
    }

    /// Record that a negative lookahead ruled out `excluded`, which matched
    /// the text at `range`.
    pub(crate) fn not_followed_by(&mut self, excluded: fn() -> Expr, range: Range<usize>) {
        self.expect(range.start, || Expectation::NotFollowedBy {
            excluded: Lazy(excluded),
            range,
        });
    }

    /// Note that `pos` was reached, and, at [`Max`], what was expected there:
    /// built only then, so that the parse keeps nothing for it. A
    /// hand-written grammar calls this where it fails on its own.
    #[inline(always)]
    pub fn expect(&mut self, pos: usize, expectation: impl FnOnce() -> Expectation) {
        L::expect(&mut self.0, pos, expectation);
    }

    /// Record that rule `node` rejected the text at `range`, which must
    /// `requirement`: a `&'static str`, or a `String` built from what was
    /// parsed.
    #[inline(always)]
    pub fn reject(
        &mut self,
        node: &'static str,
        range: Range<usize>,
        requirement: impl Into<Cow<'static, str>>,
    ) {
        self.expect(range.end, || Expectation::Rejected {
            node,
            range,
            requirement: requirement.into(),
        });
    }
}

/// Run `f` tracing nothing: a lookahead's failure isn't the parse's.
pub(crate) fn probe<R>(f: impl FnOnce(State<'_, Min>) -> R) -> R {
    f(State(()))
}

/// How much a parse traces, and so what its [`Error`] reports: [`Min`],
/// [`Pos`] or [`Max`]. Orthogonal to [`Success`], which says what it builds.
pub trait Level: Sized + 'static + private::Sealed {
    /// Whether this is [`Max`]: then every alternative is tried, so that each
    /// says what it expected, rather than skipped when it can't match.
    const IS_MAX: bool;

    /// What a [`State`] at this level carries.
    #[doc(hidden)]
    type State<'a>;

    /// What an [`Error`] at this level carries.
    #[doc(hidden)]
    type Error: Clone + fmt::Debug + fmt::Display + PartialEq + Eq;

    #[doc(hidden)]
    fn reborrow<'s, 'a: 's>(state: &'s mut Self::State<'a>) -> Self::State<'s>;

    #[doc(hidden)]
    fn enter<'s, 'a: 's>(state: &'s mut Self::State<'a>, frame: Frame) -> Self::State<'s>;

    #[doc(hidden)]
    fn expect(state: &mut Self::State<'_>, pos: usize, expectation: impl FnOnce() -> Expectation);

    /// Parse `T` at the start of `input`, to its end if `whole`: what the
    /// entry points do.
    #[doc(hidden)]
    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Self>>;
}

/// The tracing levels, least to most: [`Min`], [`Pos`], [`Max`]. With
/// `use tygr::*`, a call reads `parse_with::<success::Value, level::Max>`.
pub mod level {
    /// The [`Level`](crate::Level) tracing nothing: the fastest parse, whose
    /// [`Error`](crate::Error) says only that it failed.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Min;

    /// The [`Level`](crate::Level) tracking the furthest position reached,
    /// which its [`Error`](crate::Error) reports.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Pos;

    /// The [`Level`](crate::Level) reporting where the parse failed and what
    /// was expected there, in the chain of rules entered: the default
    /// [`Error`](crate::Error). A failure costs a second parse.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Max;
}

impl Level for Min {
    const IS_MAX: bool = false;
    type State<'a> = ();
    type Error = Failed;

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(_state: &'s mut ()) {}

    #[inline(always)]
    fn enter<'s, 'a: 's>(_state: &'s mut (), _frame: Frame) {}

    #[inline(always)]
    fn expect(_state: &mut (), _pos: usize, _expectation: impl FnOnce() -> Expectation) {}

    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Min>> {
        pass::<T, S, Min>(input, whole, State(())).ok_or(Error(Failed))
    }
}

impl Level for Pos {
    const IS_MAX: bool = false;
    type State<'a> = &'a mut usize;
    type Error = FailedAt;

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(state: &'s mut &'a mut usize) -> &'s mut usize {
        state
    }

    #[inline(always)]
    fn enter<'s, 'a: 's>(state: &'s mut &'a mut usize, _frame: Frame) -> &'s mut usize {
        state
    }

    #[inline(always)]
    fn expect(state: &mut &mut usize, pos: usize, _expectation: impl FnOnce() -> Expectation) {
        if pos > **state {
            **state = pos;
        }
    }

    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Pos>> {
        let mut pos = 0;
        pass::<T, S, Pos>(input, whole, State(&mut pos)).ok_or(Error(FailedAt(pos)))
    }
}

/// [`Max`]'s state: the error being built and the rules entered so far, a
/// list through the stack.
#[doc(hidden)]
pub struct Recorder<'a> {
    error: &'a mut Traced,
    frames: Option<Frames<'a>>,
}

impl Level for Max {
    const IS_MAX: bool = true;
    type State<'a> = Recorder<'a>;
    type Error = Traced;

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(state: &'s mut Recorder<'a>) -> Recorder<'s> {
        Recorder {
            error: state.error,
            frames: state.frames,
        }
    }

    #[inline(always)]
    fn enter<'s, 'a: 's>(state: &'s mut Recorder<'a>, frame: Frame) -> Recorder<'s> {
        Recorder {
            error: state.error,
            frames: Some(Frames {
                frame,
                parent: state.frames.as_ref(),
            }),
        }
    }

    #[inline(always)]
    fn expect(state: &mut Recorder<'_>, pos: usize, expectation: impl FnOnce() -> Expectation) {
        if pos == state.error.pos {
            record(state, expectation);
        }
    }

    fn run<T: Parse, S: Success>(input: &str, whole: bool) -> Result<(S::Output<T>, usize), Error> {
        let mut error = Traced {
            pos: 0,
            traces: Vec::new(),
        };
        if let Some(result) = pass::<T, S, Pos>(input, whole, State(&mut error.pos)) {
            return Ok(result);
        }
        let recorder = Recorder {
            error: &mut error,
            frames: None,
        };
        pass::<T, Unit, Max>(input, whole, State(recorder));
        Err(Error(error))
    }
}

/// Record `expectation` at the furthest position, in the rules entered.
#[cold]
#[inline(never)]
fn record(state: &mut Recorder<'_>, expectation: impl FnOnce() -> Expectation) {
    let frames = std::iter::successors(state.frames.as_ref(), |frames| frames.parent)
        .map(|frames| frames.frame);
    state.error.record(frames, expectation());
}

/// One pass: `T` at the start of `input` and, if `whole`, its end.
fn pass<T: Parse, S: Success, L: Level>(
    input: &str,
    whole: bool,
    mut state: State<'_, L>,
) -> Option<(S::Output<T>, usize)> {
    let (value, end) = T::parse_at::<S, L>(input, 0, state.reborrow())?;
    (!whole || state.at_end(input, end)).then_some((value, end))
}

mod private {
    /// Only [`Min`](super::level::Min), [`Pos`](super::level::Pos) and [`Max`](super::level::Max)
    /// are levels.
    pub trait Sealed {}
    impl Sealed for super::level::Min {}
    impl Sealed for super::level::Pos {}
    impl Sealed for super::level::Max {}
}

/// A parse failure, traced at level `L`: that it failed ([`Min`]), where
/// ([`Pos`]), or where and what was expected there ([`Max`], the default).
pub struct Error<L: Level = Max>(L::Error);

impl Error<Pos> {
    /// The furthest byte offset reached; [`OLC`](crate::OLC) maps it to a
    /// `(line, character)`.
    pub fn pos(&self) -> usize {
        self.0.0
    }
}

impl Error {
    /// The furthest byte offset reached; [`OLC`](crate::OLC) maps it to a
    /// `(line, character)`.
    pub fn pos(&self) -> usize {
        self.0.pos
    }

    /// What was expected at [`pos`](Self::pos), without repeats.
    pub fn traces(&self) -> &[Trace] {
        &self.0.traces
    }
}

impl<L: Level> Clone for Error<L> {
    fn clone(&self) -> Self {
        Error(self.0.clone())
    }
}

impl<L: Level> PartialEq for Error<L> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<L: Level> Eq for Error<L> {}

impl<L: Level> fmt::Display for Error<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl<L: Level> fmt::Debug for Error<L> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl<L: Level> std::error::Error for Error<L> {}

/// [`Min`]'s error: only that the parse failed.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct Failed;

impl fmt::Display for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("parse error")
    }
}

impl fmt::Debug for Failed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// [`Pos`]'s error: the furthest byte offset reached.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct FailedAt(usize);

impl fmt::Display for FailedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}", self.0)
    }
}

impl fmt::Debug for FailedAt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// [`Max`]'s error: the furthest byte offset reached, and what was expected
/// there.
#[doc(hidden)]
#[derive(Clone, PartialEq, Eq)]
pub struct Traced {
    pos: usize,
    traces: Vec<Trace>,
}

impl Traced {
    /// Record `expectation` in `frames` (innermost first), unless it's
    /// recorded there already: a backtracking grammar can expect the same few
    /// things exponentially often.
    fn record(&mut self, frames: impl Iterator<Item = Frame> + Clone, expectation: Expectation) {
        let recorded = self.traces.iter().any(|trace| {
            trace.expectation == expectation
                && trace.context.iter().rev().copied().eq(frames.clone())
        });
        if !recorded {
            let mut context: Vec<Frame> = frames.collect();
            context.reverse();
            self.traces.push(Trace {
                context,
                expectation,
            });
        }
    }
}

impl fmt::Display for Traced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}", self.pos)?;
        let mut lines: Vec<String> = self
            .traces
            .iter()
            .map(|trace| match (trace.context.last(), &trace.expectation) {
                // A rule that starts here stands for the terminals it expected.
                (Some(frame), Expectation::Token { .. } | Expectation::CharClass(_))
                    if frame.pos == self.pos =>
                {
                    frame.node.to_string()
                }
                _ => trace.expectation.to_string(),
            })
            .collect();
        lines.sort();
        lines.dedup();
        if !lines.is_empty() {
            write!(f, ", expected:")?;
            for line in lines {
                write!(f, "\n\t- {line}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Traced {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}", self.pos)?;
        if !self.traces.is_empty() {
            write!(f, ", expected:")?;
        }
        for trace in &self.traces {
            write!(f, "\n\t- ")?;
            for Frame { node, pos } in &trace.context {
                write!(f, "{node}@{pos}/")?;
            }
            write!(f, "{}", trace.expectation)?;
        }
        Ok(())
    }
}

/// The rules entered so far, innermost first: a list through the stack, as
/// [`Max`]'s state.
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Frames<'a> {
    pub(crate) frame: Frame,
    pub(crate) parent: Option<&'a Frames<'a>>,
}

/// What was expected at [`Error::pos`].
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expectation {
    /// A literal (`StringEq!` / `StringEqCI!`) matched `range`, then
    /// diverged: it's recorded at the range's end.
    Token {
        /// The literal.
        text: &'static str,
        /// What it matched; empty if it diverged at once.
        range: Range<usize>,
    },
    /// A char of the [`CharClass`](crate::CharClass) with this name;
    /// `"any"` for `char`.
    CharClass(&'static str),
    /// The text at `range` matched rule `node`, which rejected it: a
    /// `#[tygr(validated)]` check or a hand-written grammar's
    /// [`State::reject`].
    Rejected {
        /// The rule's name.
        node: &'static str,
        /// The bytes rejected.
        range: Range<usize>,
        /// What they must do: a
        /// [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT), or what
        /// the grammar passed to [`State::reject`].
        requirement: Cow<'static, str>,
    },
    /// The text at `range` matched, but a `#[tygr(try_from = S)]` or
    /// `from_str` conversion of it failed.
    Convertible {
        /// The type converted into, as written in the attribute's field or
        /// type: not a rule's `name`, which the trace's innermost frame has.
        into: &'static str,
        /// The bytes that didn't convert.
        range: Range<usize>,
        /// The conversion error's `Display`.
        error: Cow<'static, str>,
    },
    /// Anything but `excluded`, which a
    /// [`NotFollowedBy`](crate::NotFollowedBy) rules out and which matched
    /// the text at `range`.
    NotFollowedBy {
        /// The excluded grammar, as EBNF.
        excluded: Lazy,
        /// What it matched.
        range: Range<usize>,
    },
    /// The end of input: the grammar matched only part of it.
    End,
}

impl fmt::Display for Expectation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expectation::Token { text, range } => {
                write!(f, "\"{}\"", escape(text, '"'))?;
                if !range.is_empty() {
                    write!(f, " at {range:?}")?;
                }
                Ok(())
            }
            Expectation::CharClass(c) => write!(f, "'{}'", escape(c, '\'')),
            Expectation::Rejected {
                node,
                range,
                requirement,
            } => write!(f, "{node} at {range:?} must {requirement}"),
            Expectation::Convertible { into, range, error } => {
                write!(
                    f,
                    "Text at {range:?} must be convertible to {into}: {error}"
                )
            }
            Expectation::NotFollowedBy { excluded, range } => match (excluded.0)() {
                Expr::Hidden => write!(f, "anything but the text at {range:?}"),
                excluded => {
                    write!(f, "anything but {excluded} at {range:?}")?;
                    for (i, text) in excluded.side_conditions().into_iter().enumerate() {
                        write!(f, " (^{}: {text})", i + 1)?;
                    }
                    Ok(())
                }
            },
            Expectation::End => write!(f, "end of input"),
        }
    }
}

/// One thing that would have let the parse continue at [`Error::pos`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Trace {
    /// The enclosing rules, outermost first.
    pub context: Vec<Frame>,
    /// What was expected at this position.
    pub expectation: Expectation,
}

/// One entry in a [`Trace`]'s [`context`](Trace::context).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The rule's name.
    pub node: &'static str,
    /// Where it was entered.
    pub pos: usize,
}
