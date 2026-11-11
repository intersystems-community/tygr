//! The state threaded through parsing, and the [`Error`] it traces.

use crate::bnf::{Expr, Lazy, escape};
#[cfg(feature = "trace_pos")]
use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

/// Parse state threaded through `parse_at`; a ZST when nothing is traced.
#[doc(hidden)]
pub struct State<'a> {
    #[cfg(feature = "trace_pos")]
    history: &'a mut History,
    #[cfg(feature = "trace_one_node")]
    context: Context<'a>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> State<'a> {
    fn new(
        #[cfg(feature = "trace_pos")] history: &'a mut History,
        #[cfg(feature = "trace_one_node")] context: Context<'a>,
    ) -> Self {
        Self {
            #[cfg(feature = "trace_pos")]
            history,
            #[cfg(feature = "trace_one_node")]
            context,
            _marker: PhantomData,
        }
    }

    /// Reborrow for a nested call.
    pub fn reborrow(&mut self) -> State<'_> {
        State {
            #[cfg(feature = "trace_pos")]
            history: &mut *self.history,
            #[cfg(feature = "trace_one_node")]
            context: self.context,
            _marker: PhantomData,
        }
    }

    /// Enter rule `node` at `pos`.
    pub fn node(
        &mut self,
        #[allow(unused_variables)] node: &'static str,
        #[allow(unused_variables)] pos: usize,
    ) -> State<'_> {
        State {
            #[cfg(feature = "trace_pos")]
            history: &mut *self.history,
            #[cfg(feature = "trace_one_node")]
            context: self.context.node(node, pos),
            _marker: PhantomData,
        }
    }

    /// Whether `pos` is the end of `input`; if not, records that it was
    /// expected.
    pub(crate) fn at_end(&mut self, input: &str, pos: usize) -> bool {
        let end = pos == input.len();
        if !end {
            self.expect(pos, Expectation::End);
        }
        end
    }

    /// Record that a negative lookahead ruled out `excluded`, which matched
    /// the text at `range`.
    pub(crate) fn not_followed_by(&mut self, excluded: fn() -> Expr, range: Range<usize>) {
        self.expect(
            range.start,
            Expectation::NotFollowedBy {
                excluded: Lazy(excluded),
                range,
            },
        );
    }

    /// Record what was expected at `pos`; nothing, without `trace_pos`.
    #[cfg_attr(not(feature = "trace_one_node"), allow(unused_variables))]
    #[inline]
    pub(crate) fn expect(&mut self, pos: usize, expectation: Expectation) {
        #[cfg(feature = "trace_pos")]
        self.history.expect(
            #[cfg(feature = "trace_one_node")]
            self.context,
            pos,
            #[cfg(feature = "trace_one_node")]
            expectation,
        );
    }

    /// Record that the text at `range` was rejected, as it must
    /// `requirement`: by validated rule `node`, or by a field conversion.
    #[inline]
    pub fn reject(
        &mut self,
        node: Option<&'static str>,
        range: Range<usize>,
        requirement: &'static str,
    ) {
        self.expect(
            range.end,
            Expectation::Rejected {
                node,
                range,
                requirement,
            },
        );
    }

    /// Run `f` without recording anything, so it leaves no trace.
    pub(crate) fn probe<R>(&mut self, f: impl FnOnce(State<'_>) -> R) -> R {
        // Already past every position, so it ignores all records.
        #[cfg(feature = "trace_pos")]
        let mut ignored = History::new(usize::MAX);
        f(State::new(
            #[cfg(feature = "trace_pos")]
            &mut ignored,
            #[cfg(feature = "trace_one_node")]
            self.context,
        ))
    }
}

/// Run a parse from a fresh state, turning `None` into the traced `Error`.
pub(crate) fn run<R>(parse: impl FnOnce(State<'_>) -> Option<R>) -> Result<R, Error> {
    #[cfg(feature = "trace_pos")]
    let mut history = History::new(0);
    let result = parse(State::new(
        #[cfg(feature = "trace_pos")]
        &mut history,
        #[cfg(feature = "trace_one_node")]
        Context::default(),
    ));
    match result {
        Some(value) => Ok(value),
        #[cfg(feature = "trace_pos")]
        None => Err(history.into_error()),
        #[cfg(not(feature = "trace_pos"))]
        None => Err(Error {}),
    }
}

/// What was expected at the furthest position reached.
#[cfg(feature = "trace_pos")]
#[derive(Debug)]
struct History {
    #[cfg(feature = "trace_one_node")]
    traces: Vec<Pending>,
    pos: usize,
}

/// A recorded trace, with its context kept cheap until the parse fails.
#[cfg(feature = "trace_one_node")]
#[derive(Debug, PartialEq)]
struct Pending {
    frames: Frames,
    expectation: Expectation,
}

/// The nearest rule, or (with `trace_all_nodes`) all of them, innermost first.
#[cfg(all(feature = "trace_one_node", not(feature = "trace_all_nodes")))]
type Frames = Option<Frame>;
#[cfg(feature = "trace_all_nodes")]
type Frames = Vec<Frame>;

#[cfg(feature = "trace_pos")]
impl History {
    /// Nothing recorded yet, and records before `pos` are ignored.
    fn new(pos: usize) -> Self {
        History {
            #[cfg(feature = "trace_one_node")]
            traces: Vec::new(),
            pos,
        }
    }

    #[inline]
    fn expect(
        &mut self,
        #[cfg(feature = "trace_one_node")] context: Context<'_>,
        other_pos: usize,
        #[cfg(feature = "trace_one_node")] expectation: Expectation,
    ) {
        #[cfg(feature = "trace_one_node")]
        let trace = || Pending {
            frames: context.frames(),
            expectation,
        };
        match self.pos.cmp(&other_pos) {
            Ordering::Equal => {
                #[cfg(feature = "trace_one_node")]
                self.traces.push(trace());
            }
            Ordering::Less => {
                self.pos = other_pos;
                #[cfg(feature = "trace_one_node")]
                {
                    // Reuse the buffer: superseded traces are dropped in place.
                    self.traces.clear();
                    self.traces.push(trace());
                }
            }
            Ordering::Greater => (),
        }
    }

    /// Drop repeated traces, keeping the first of each in order: a
    /// backtracking grammar can expect the same few things exponentially
    /// often. Not on each record: the JSON benchmark makes 1.5 records per
    /// byte, and one more instruction there costs it 15%.
    #[cfg(feature = "trace_one_node")]
    fn dedup(&mut self) {
        let mut kept = 0;
        for i in 0..self.traces.len() {
            if !self.traces[..kept].contains(&self.traces[i]) {
                self.traces.swap(kept, i);
                kept += 1;
            }
        }
        self.traces.truncate(kept);
    }

    fn into_error(#[allow(unused_mut)] mut self) -> Error {
        #[cfg(feature = "trace_one_node")]
        self.dedup();
        #[cfg(feature = "trace_one_node")]
        let traces = self
            .traces
            .into_iter()
            .map(
                |Pending {
                     frames,
                     expectation,
                 }| {
                    let mut context: Vec<Frame> = frames.into_iter().collect();
                    context.reverse();
                    Trace {
                        context,
                        expectation,
                    }
                },
            )
            .collect();
        Error {
            #[cfg(feature = "trace_one_node")]
            traces,
            pos: self.pos,
        }
    }
}

/// One candidate for what would have matched at a [`Trace`]'s recorded position.
#[derive(Debug, PartialEq, Eq)]
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
    /// No character satisfying this [`CharClass`](crate::CharClass) was found.
    CharClass(&'static str),
    /// The text at `range` matched but was rejected: a
    /// `#[tygr(validated)]` check or a field conversion failed.
    Rejected {
        /// The validated rule's name; `None` for a field conversion.
        node: Option<&'static str>,
        /// The bytes rejected.
        range: Range<usize>,
        /// What they must do: a
        /// [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT), or
        /// `"be convertible to T"` for a field of type `T`.
        requirement: &'static str,
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
            Expectation::NotFollowedBy { excluded, range } => {
                write!(f, "anything but {excluded} at {range:?}")
            }
            Expectation::End => write!(f, "end of input"),
        }
    }
}

/// One thing that would have let the parse continue at [`Error::pos`].
#[derive(Debug, PartialEq, Eq)]
pub struct Trace {
    /// Enclosing rules, outermost first (all of them only with
    /// `trace_all_nodes`).
    pub context: Vec<Frame>,
    /// What was expected at this position.
    pub expectation: Expectation,
}

/// The rule path to the current position.
#[cfg(feature = "trace_one_node")]
#[derive(Clone, Copy, Default)]
struct Context<'a>(Option<ContextNode<'a>>);

#[cfg(feature = "trace_one_node")]
#[derive(Clone, Copy)]
struct ContextNode<'a> {
    first: Frame,
    _marker: PhantomData<&'a ()>,
    #[cfg(feature = "trace_all_nodes")]
    remaining: &'a Context<'a>,
}

#[cfg(feature = "trace_one_node")]
impl<'a> Context<'a> {
    fn frames(self) -> Frames {
        #[cfg(feature = "trace_all_nodes")]
        {
            let mut result = vec![];
            let mut cursor = self;
            while let Some(node) = cursor.0 {
                result.push(node.first);
                cursor = *node.remaining;
            }
            result
        }
        #[cfg(not(feature = "trace_all_nodes"))]
        {
            self.0.map(|node| node.first)
        }
    }

    fn node(&'a self, node: &'static str, pos: usize) -> Context<'a> {
        Context(Some(ContextNode {
            first: Frame { node, pos },
            _marker: PhantomData,
            #[cfg(feature = "trace_all_nodes")]
            remaining: self,
        }))
    }
}

/// One entry in a [`Trace`]'s [`context`](Trace::context).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The rule's name.
    pub node: &'static str,
    /// Where it was entered.
    pub pos: usize,
}

/// A parse failure. Its detail depends on the `trace_*` features; read it
/// through [`Error::pos`] and [`Error::traces`].
#[derive(PartialEq, Eq)]
pub struct Error {
    #[cfg(feature = "trace_one_node")]
    traces: Vec<Trace>,
    #[cfg(feature = "trace_pos")]
    pos: usize,
}

impl Error {
    /// What was expected at [`pos`](Self::pos). Empty without
    /// `trace_one_node`.
    pub fn traces(&self) -> &[Trace] {
        #[cfg(feature = "trace_one_node")]
        {
            &self.traces
        }
        #[cfg(not(feature = "trace_one_node"))]
        {
            &[]
        }
    }

    /// The deepest byte offset reached. `0` without `trace_pos`.
    pub fn pos(&self) -> usize {
        #[cfg(feature = "trace_pos")]
        {
            self.pos
        }
        #[cfg(not(feature = "trace_pos"))]
        {
            0
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error")?;
        #[cfg(feature = "trace_pos")]
        write!(f, " at byte {}", self.pos)?;
        #[cfg(feature = "trace_one_node")]
        {
            let mut lines: Vec<String> = self
                .traces
                .iter()
                .map(
                    |Trace {
                         context,
                         expectation,
                     }| {
                        // A rule that starts here stands for what it expected.
                        match context.last() {
                            Some(frame) if frame.pos == self.pos => frame.node.to_string(),
                            _ => format!("{expectation}"),
                        }
                    },
                )
                .collect();
            lines.sort();
            lines.dedup();
            if !lines.is_empty() {
                write!(f, ", expected:")?;
                for line in lines {
                    write!(f, "\n\t- {line}")?;
                }
            }
        }
        Ok(())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "trace_one_node")]
        {
            writeln!(f, ":{}", self.pos)?;
            writeln!(f, "Looking for")?;
            for trace in &self.traces {
                writeln!(
                    f,
                    "\t- {}!{}",
                    trace
                        .context
                        .iter()
                        .map(|Frame { node, pos }| { format!("{node}@:{pos}") })
                        .collect::<Vec<String>>()
                        .join("/"),
                    trace.expectation
                )?;
            }
            Ok(())
        }
        // Without traces, that's all there is to say.
        #[cfg(not(feature = "trace_one_node"))]
        {
            fmt::Display::fmt(self, f)
        }
    }
}

impl std::error::Error for Error {}
