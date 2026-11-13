//! The state threaded through parsing, and the [`Error`] it traces.

use crate::bnf::{Expr, Lazy, escape};
use crate::grammar::{Check, Mode};
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

/// Parse state threaded through [`Parse::parse_at`](crate::Parse::parse_at),
/// in mode `M`. A parse runs once, tracking only the furthest position it
/// reaches; if it fails, it runs again in [`Record`](crate::Record) mode,
/// recording what it expected there. A hand-written grammar enters its rule
/// with [`node`](Self::node), in [`fail_at`](crate::Parse::fail_at) too, and
/// passes a [`reborrow`](Self::reborrow) to each grammar it delegates to.
pub struct State<'a, M: Mode> {
    history: &'a mut History,
    /// The rules entered, when recording.
    context: M::Context<'a>,
}

impl<'a, M: Mode> State<'a, M> {
    fn new(history: &'a mut History) -> Self {
        State {
            history,
            context: Default::default(),
        }
    }

    /// Reborrow for a nested call.
    #[inline(always)]
    pub fn reborrow(&mut self) -> State<'_, M> {
        State {
            history: &mut *self.history,
            context: M::reborrow(self.context),
        }
    }

    /// Enter rule `node` at `pos`, so that what's recorded inside names it.
    #[inline(always)]
    pub fn node(&mut self, node: &'static str, pos: usize) -> State<'_, M> {
        State {
            history: &mut *self.history,
            context: M::enter(&self.context, Frame { node, pos }),
        }
    }

    /// Whether `pos` is the end of `input`; if not, records that it was
    /// expected.
    pub(crate) fn at_end(&mut self, input: &str, pos: usize) -> bool {
        let end = pos == input.len();
        if !end {
            self.expect(pos, || Recorded::End);
        }
        end
    }

    /// Record that a negative lookahead ruled out `excluded`, which matched
    /// the text at `range`.
    pub(crate) fn not_followed_by(&mut self, excluded: fn() -> Expr, range: Range<usize>) {
        self.expect(range.start, || Recorded::NotFollowedBy {
            excluded: Lazy(excluded),
            range,
        });
    }

    /// Note that `pos` was reached; recording, record what was expected
    /// there if it's the furthest position. The expectation is built only
    /// then, out of line, so that the parse keeps nothing for it.
    #[inline(always)]
    pub(crate) fn expect(&mut self, pos: usize, expectation: impl FnOnce() -> Recorded) {
        if !M::RECORD {
            self.history.reach(pos);
        } else if pos == self.history.pos {
            record::<M>(self.history, self.context, expectation);
        }
    }

    /// Record that the text at `range` was rejected, as it must
    /// `requirement`: by validated rule `node`, or by a conversion. A
    /// `String` requirement, built from what was parsed, is kept for the
    /// error; test [`Mode::RECORD`] first to build it only when it's kept.
    #[inline(always)]
    pub fn reject(
        &mut self,
        node: Option<&'static str>,
        range: Range<usize>,
        requirement: impl Into<Cow<'static, str>>,
    ) {
        if !M::RECORD {
            self.history.reach(range.end);
        } else if range.end == self.history.pos {
            record_rejected::<M>(self.history, self.context, node, range, requirement.into());
        }
    }

    /// This state in the mode that builds values, for a part whose value is
    /// needed whatever the mode: [`Parse::build_at`](crate::Parse::build_at)
    /// parses one.
    pub fn built(self) -> State<'a, M::Built> {
        State {
            history: self.history,
            context: self.context.into(),
        }
    }

    /// This state in the mode that only checks, for a part whose value isn't
    /// needed: [`Parse::check_at`](crate::Parse::check_at) parses one.
    pub fn checked(self) -> State<'a, M::Checked> {
        State {
            history: self.history,
            context: self.context.into(),
        }
    }

    /// Run `f` without recording anything.
    pub(crate) fn probe<R>(&mut self, f: impl FnOnce(State<'_, Check>) -> R) -> R {
        // Already past every position, so nothing is recorded.
        let mut ignored = History::new(usize::MAX);
        f(State::new(&mut ignored))
    }
}

/// Record `expectation` at the furthest position, in the rules entered.
#[cold]
#[inline(never)]
fn record<M: Mode>(
    history: &mut History,
    context: M::Context<'_>,
    expectation: impl FnOnce() -> Recorded,
) {
    history.record(M::frames(&context), expectation());
}

#[cold]
#[inline(never)]
fn record_rejected<M: Mode>(
    history: &mut History,
    context: M::Context<'_>,
    node: Option<&'static str>,
    range: Range<usize>,
    requirement: Cow<'static, str>,
) {
    let requirement = match requirement {
        Cow::Borrowed(text) => Requirement::Static(text),
        Cow::Owned(text) => Requirement::Owned(history.own(text)),
    };
    let expectation = Recorded::Rejected {
        node,
        range,
        requirement,
    };
    history.record(M::frames(&context), expectation);
}

/// Run a parse: `parse` tracks the furthest position it reaches; if it
/// fails, `record`, the same parse in [`Record`](crate::Record) mode, records
/// what was expected there, for the `Error`.
pub(crate) fn run<M: Mode, R>(
    parse: impl FnOnce(State<'_, M>) -> Option<R>,
    record: impl FnOnce(State<'_, crate::Record<M>>),
) -> Result<R, Error> {
    let mut history = History::new(0);
    if let Some(value) = parse(State::new(&mut history)) {
        return Ok(value);
    }
    record(State::new(&mut history));
    Err(history.into_error())
}

/// The rules entered so far, innermost first: a list through the stack, as
/// [`Record`](crate::Record)'s [`Mode::Context`].
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Frames<'a> {
    frame: Frame,
    parent: Option<&'a Frames<'a>>,
}

impl<'a> Frames<'a> {
    pub(crate) fn enter<'s>(parent: &'s Option<Frames<'a>>, frame: Frame) -> Option<Frames<'s>> {
        Some(Frames {
            frame,
            parent: parent.as_ref(),
        })
    }

    pub(crate) fn iter<'s>(
        context: &'s Option<Frames<'a>>,
    ) -> impl Iterator<Item = Frame> + Clone + 's {
        std::iter::successors(context.as_ref(), |frames| frames.parent).map(|frames| frames.frame)
    }
}

/// The furthest position a parse reached and, in a recording pass, what it
/// expected there.
struct History {
    /// The furthest position reached; fixed in a recording pass, which keeps
    /// only the records made there.
    pos: usize,
    traces: Vec<Pending>,
    /// Requirements built while parsing, which `Pending`s refer to by index.
    owned: Vec<String>,
}

/// A recorded trace: the rules it was made in, innermost first, and what was
/// expected.
struct Pending {
    frames: Vec<Frame>,
    expectation: Recorded,
}

/// An [`Expectation`] as recorded: a rejection's requirement is a
/// `&'static str` or an index into [`History::owned`].
#[derive(PartialEq)]
pub(crate) enum Recorded {
    Token {
        text: &'static str,
        range: Range<usize>,
    },
    CharClass(&'static str),
    Rejected {
        node: Option<&'static str>,
        range: Range<usize>,
        requirement: Requirement,
    },
    NotFollowedBy {
        excluded: Lazy,
        range: Range<usize>,
    },
    End,
}

#[derive(PartialEq)]
pub(crate) enum Requirement {
    Static(&'static str),
    Owned(usize),
}

impl Requirement {
    fn text<'a>(&'a self, owned: &'a [String]) -> &'a str {
        match self {
            Requirement::Static(text) => text,
            Requirement::Owned(i) => &owned[*i],
        }
    }
}

impl Recorded {
    /// The expectation to report, taking an owned requirement out of `owned`.
    fn into_expectation(self, owned: &mut [String]) -> Expectation {
        match self {
            Recorded::Token { text, range } => Expectation::Token { text, range },
            Recorded::CharClass(name) => Expectation::CharClass(name),
            Recorded::Rejected {
                node,
                range,
                requirement,
            } => Expectation::Rejected {
                node,
                range,
                requirement: match requirement {
                    Requirement::Static(text) => Cow::Borrowed(text),
                    Requirement::Owned(i) => Cow::Owned(std::mem::take(&mut owned[i])),
                },
            },
            Recorded::NotFollowedBy { excluded, range } => {
                Expectation::NotFollowedBy { excluded, range }
            }
            Recorded::End => Expectation::End,
        }
    }
}

impl History {
    /// Nothing recorded yet, and records before `pos` are ignored.
    fn new(pos: usize) -> Self {
        History {
            pos,
            traces: Vec::new(),
            owned: Vec::new(),
        }
    }

    /// The first pass: only the furthest position matters.
    #[inline]
    fn reach(&mut self, pos: usize) {
        if pos > self.pos {
            self.pos = pos;
        }
    }

    /// Record `expectation` in `frames` (innermost first), unless it's
    /// recorded there already: a backtracking grammar can expect the same few
    /// things exponentially often.
    fn record(&mut self, frames: impl Iterator<Item = Frame> + Clone, expectation: Recorded) {
        let recorded = self.traces.iter().any(|t| {
            self.same(&t.expectation, &expectation) && t.frames.iter().copied().eq(frames.clone())
        });
        if !recorded {
            self.traces.push(Pending {
                frames: frames.collect(),
                expectation,
            });
        }
    }

    /// Keep `text` for the error, returning its index.
    fn own(&mut self, text: String) -> usize {
        self.owned.push(text);
        self.owned.len() - 1
    }

    /// Would `a` and `b` report the same expectation? Owned requirements
    /// compare by text.
    fn same(&self, a: &Recorded, b: &Recorded) -> bool {
        match (a, b) {
            (
                Recorded::Rejected {
                    node,
                    range,
                    requirement,
                },
                Recorded::Rejected {
                    node: n,
                    range: r,
                    requirement: q,
                },
            ) => node == n && range == r && requirement.text(&self.owned) == q.text(&self.owned),
            (a, b) => a == b,
        }
    }

    fn into_error(mut self) -> Error {
        let mut owned = std::mem::take(&mut self.owned);
        let traces = self
            .traces
            .into_iter()
            .map(|pending| Trace {
                context: pending.frames.into_iter().rev().collect(),
                expectation: pending.expectation.into_expectation(&mut owned),
            })
            .collect();
        Error {
            traces,
            pos: self.pos,
        }
    }
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
    /// The text at `range` matched but was rejected: a
    /// `#[tygr(validated)]` check or a conversion failed.
    Rejected {
        /// The validated rule's name; `None` for a conversion.
        node: Option<&'static str>,
        /// The bytes rejected.
        range: Range<usize>,
        /// What they must do: a
        /// [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT),
        /// `"be convertible to T"` for a conversion into `T`, or what a
        /// hand-written grammar passed to [`State::reject`].
        requirement: Cow<'static, str>,
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
            } => write!(
                f,
                "{} at {range:?} must {requirement}",
                node.unwrap_or("Text")
            ),
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

/// A parse failure: where it got stuck, and what it expected there.
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    traces: Vec<Trace>,
    pos: usize,
}

impl Error {
    /// What was expected at [`pos`](Self::pos), without repeats.
    pub fn traces(&self) -> &[Trace] {
        &self.traces
    }

    /// The furthest byte offset reached; [`OLC`](crate::OLC) maps it to a
    /// `(line, character)`.
    pub fn pos(&self) -> usize {
        self.pos
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

impl std::error::Error for Error {}
