//! The state threaded through parsing, and the [`Error`] it builds.

use crate::bnf::{Expr, Lazy, escape};
use crate::grammar::{Parse, Success, Unit};
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

/// Parse state threaded through [`Parse::parse_at`](crate::Parse::parse_at),
/// tracing at level `L`. A hand-written grammar enters its rule with
/// [`node`](Self::node) and passes a [`reborrow`](Self::reborrow) to each
/// grammar it delegates to.
pub struct State<'a, L: Level>(L::Store<'a>);

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

    /// Note that `pos` was reached, and, when recording, what was expected
    /// there: built only then, so that the parse keeps nothing for it.
    #[inline(always)]
    pub(crate) fn expect(&mut self, pos: usize, expectation: impl FnOnce() -> Expectation) {
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

/// Note that a dispatch table found no variant starting at `pos`, where the
/// skipped ones would have failed; the recording pass tries them instead.
#[doc(hidden)]
#[inline(always)]
pub fn miss<L: Level>(state: &mut State<L>, pos: usize) {
    state.expect(pos, || unreachable!("the recording pass doesn't dispatch"));
}

/// Run `f` tracing nothing: a lookahead's failure isn't the parse's.
pub(crate) fn probe<R>(f: impl FnOnce(State<'_, Min>) -> R) -> R {
    f(State(()))
}

/// How much a parse traces, and so what its [`Error`] reports: [`Min`],
/// [`Pos`] or [`Max`]. Orthogonal to [`Success`], which says what it builds.
pub trait Level: Sized + 'static + private::Sealed {
    /// Whether what was expected is recorded: only at [`Max`], in the second
    /// pass of a parse that failed.
    const RECORD: bool;

    #[doc(hidden)]
    type Store<'a>;

    #[doc(hidden)]
    type Pos: Copy + fmt::Debug + PartialEq + Eq;

    #[doc(hidden)]
    type Traces: Clone + fmt::Debug + PartialEq + Eq;

    #[doc(hidden)]
    fn reborrow<'s, 'a: 's>(store: &'s mut Self::Store<'a>) -> Self::Store<'s>;

    #[doc(hidden)]
    fn enter<'s, 'a: 's>(store: &'s mut Self::Store<'a>, frame: Frame) -> Self::Store<'s>;

    #[doc(hidden)]
    fn expect(store: &mut Self::Store<'_>, pos: usize, expectation: impl FnOnce() -> Expectation);

    /// Parse `T` at the start of `input`, to its end if `whole`: what the
    /// entry points do.
    #[doc(hidden)]
    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Self>>;
}

/// Traces nothing: the fastest parse, whose [`Error`] says only that it
/// failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Min;

/// Tracks the furthest position reached, which its [`Error`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos;

/// Tracks the furthest position, then, if the parse failed, runs it again
/// recording what was expected there, in the chain of rules entered: the
/// [`Error`] of [`parse`](Parse::parse) and friends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Max;

impl Level for Min {
    const RECORD: bool = false;
    type Store<'a> = ();
    type Pos = ();
    type Traces = ();

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(_store: &'s mut ()) {}

    #[inline(always)]
    fn enter<'s, 'a: 's>(_store: &'s mut (), _frame: Frame) {}

    #[inline(always)]
    fn expect(_store: &mut (), _pos: usize, _expectation: impl FnOnce() -> Expectation) {}

    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Min>> {
        pass::<T, S, Min>(input, whole, State(())).ok_or(Error {
            pos: (),
            traces: (),
        })
    }
}

impl Level for Pos {
    const RECORD: bool = false;
    type Store<'a> = &'a mut usize;
    type Pos = usize;
    type Traces = ();

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(store: &'s mut &'a mut usize) -> &'s mut usize {
        store
    }

    #[inline(always)]
    fn enter<'s, 'a: 's>(store: &'s mut &'a mut usize, _frame: Frame) -> &'s mut usize {
        store
    }

    #[inline(always)]
    fn expect(store: &mut &mut usize, pos: usize, _expectation: impl FnOnce() -> Expectation) {
        if pos > **store {
            **store = pos;
        }
    }

    fn run<T: Parse, S: Success>(
        input: &str,
        whole: bool,
    ) -> Result<(S::Output<T>, usize), Error<Pos>> {
        let mut pos = 0;
        pass::<T, S, Pos>(input, whole, State(&mut pos)).ok_or(Error { pos, traces: () })
    }
}

/// [`Max`]'s store: the error being built and the rules entered so far, a
/// list through the stack.
#[doc(hidden)]
pub struct Recorder<'a> {
    error: &'a mut Error,
    frames: Option<Frames<'a>>,
}

impl Level for Max {
    const RECORD: bool = true;
    type Store<'a> = Recorder<'a>;
    type Pos = usize;
    type Traces = Vec<Trace>;

    #[inline(always)]
    fn reborrow<'s, 'a: 's>(store: &'s mut Recorder<'a>) -> Recorder<'s> {
        Recorder {
            error: store.error,
            frames: store.frames,
        }
    }

    #[inline(always)]
    fn enter<'s, 'a: 's>(store: &'s mut Recorder<'a>, frame: Frame) -> Recorder<'s> {
        Recorder {
            error: store.error,
            frames: Some(Frames {
                frame,
                parent: store.frames.as_ref(),
            }),
        }
    }

    #[inline(always)]
    fn expect(store: &mut Recorder<'_>, pos: usize, expectation: impl FnOnce() -> Expectation) {
        if pos == store.error.pos {
            record(store, expectation);
        }
    }

    fn run<T: Parse, S: Success>(input: &str, whole: bool) -> Result<(S::Output<T>, usize), Error> {
        let mut error = Error {
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
        Err(error)
    }
}

/// Record `expectation` at the furthest position, in the rules entered.
#[cold]
#[inline(never)]
fn record(store: &mut Recorder<'_>, expectation: impl FnOnce() -> Expectation) {
    let frames = std::iter::successors(store.frames.as_ref(), |frames| frames.parent)
        .map(|frames| frames.frame);
    store.error.record(frames, expectation());
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
    /// Only [`Min`](super::Min), [`Pos`](super::Pos) and [`Max`](super::Max)
    /// are levels.
    pub trait Sealed {}
    impl Sealed for super::Min {}
    impl Sealed for super::Pos {}
    impl Sealed for super::Max {}
}

/// A parse failure, traced at level `L`: that it failed ([`Min`]), where
/// ([`Pos`]), or where and what was expected there ([`Max`], the default).
pub struct Error<L: Level = Max> {
    pos: L::Pos,
    traces: L::Traces,
}

impl<L: Level<Pos = usize>> Error<L> {
    /// The furthest byte offset reached; [`OLC`](crate::OLC) maps it to a
    /// `(line, character)`.
    pub fn pos(&self) -> usize {
        self.pos
    }
}

impl<L: Level<Traces = Vec<Trace>>> Error<L> {
    /// What was expected at [`pos`](Self::pos), without repeats.
    pub fn traces(&self) -> &[Trace] {
        &self.traces
    }
}

impl Error {
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

impl<L: Level> Clone for Error<L> {
    fn clone(&self) -> Self {
        Error {
            pos: self.pos,
            traces: self.traces.clone(),
        }
    }
}

impl<L: Level> PartialEq for Error<L> {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.traces == other.traces
    }
}

impl<L: Level> Eq for Error<L> {}

impl<L: Level> std::error::Error for Error<L> where Self: fmt::Debug + fmt::Display {}

impl fmt::Display for Error<Min> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("parse error")
    }
}

impl fmt::Debug for Error<Min> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Error<Pos> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at byte {}", self.pos)
    }
}

impl fmt::Debug for Error<Pos> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Error {
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

impl fmt::Debug for Error {
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
/// [`Max`]'s store.
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
