//! The state threaded through parsing, and the [`Error`] it builds.

use crate::bnf::{Expr, Lazy, escape};
use std::borrow::Cow;
use std::fmt;
use std::ops::Range;

/// The rules entered so far, innermost first: a list through the stack, as
/// [`Record`]'s [`Mode::Context`].
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

/// A parse failure: where it got stuck, and what it expected there.
#[derive(Clone, PartialEq, Eq)]
pub struct Error {
    traces: Vec<Trace>,
    pos: usize,
}

impl Error {
    /// No position reached yet.
    fn new() -> Self {
        Error {
            traces: Vec::new(),
            pos: 0,
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
