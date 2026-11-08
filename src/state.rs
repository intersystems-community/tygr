#[cfg(feature = "trace_one_node")]
use crate::bnf::escape_string;
#[cfg(feature = "trace_pos")]
use std::cmp::Ordering;
use std::{fmt::Display, marker::PhantomData, ops::Range};

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

    /// Record what was expected at `other_pos`.
    #[cfg(feature = "trace_pos")]
    #[inline]
    pub fn expect(
        &mut self,
        other_pos: usize,
        #[cfg(feature = "trace_one_node")] expectation: Expectation,
    ) {
        self.history.expect(
            #[cfg(feature = "trace_one_node")]
            self.context,
            other_pos,
            #[cfg(feature = "trace_one_node")]
            expectation,
        );
    }

    /// Run `f` with a throwaway history, so it leaves no trace.
    pub(crate) fn probe<R>(&mut self, f: impl FnOnce(State<'_>) -> R) -> R {
        #[cfg(feature = "trace_pos")]
        let mut scratch = History::default();
        f(State::new(
            #[cfg(feature = "trace_pos")]
            &mut scratch,
            #[cfg(feature = "trace_one_node")]
            self.context,
        ))
    }
}

/// Run a parse from a fresh state, turning `None` into the traced `Error`.
pub(crate) fn run<R>(parse: impl FnOnce(State<'_>) -> Option<R>) -> Result<R, Error> {
    #[cfg(feature = "trace_pos")]
    let mut history = History::default();
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
#[derive(std::fmt::Debug, Default)]
struct History {
    #[cfg(feature = "trace_one_node")]
    traces: Vec<Pending>,
    pos: usize,
}

/// A recorded trace, with its context kept cheap until the parse fails.
#[cfg(feature = "trace_one_node")]
#[derive(std::fmt::Debug)]
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

    fn into_error(self) -> Error {
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
#[derive(std::fmt::Debug, PartialEq, Eq)]
pub enum Expectation {
    /// This literal (`StringEq!` / `StringEqCI!`) didn't match.
    Token(&'static [u8]),
    /// No character satisfying this [`CharClass`](crate::CharClass) was found.
    CharClass(&'static str),
    /// A rule matched `range` but rejected the result: a
    /// `#[tygr(validated)]` check or a `GrammarFrom*` conversion failed.
    Rejected {
        /// The rule's [`GrammarRule::NAME`](crate::GrammarRule::NAME).
        node: &'static str,
        /// The bytes it matched.
        range: Range<usize>,
        /// What the result must do: a
        /// [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT), or
        /// `"be convertible"`.
        requirement: &'static str,
    },
}

#[cfg(feature = "trace_one_node")]
impl Display for Expectation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expectation::Token(t) => {
                write!(f, "\"{}\"", escape_string(&String::from_utf8_lossy(t)))
            }
            Expectation::CharClass(c) => write!(f, "Char of {c}"),
            Expectation::Rejected {
                node,
                range,
                requirement,
            } => write!(f, "{node} at {range:?} must {requirement}"),
        }
    }
}

/// One thing that would have let the parse continue at [`Error::pos`].
#[derive(std::fmt::Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, std::fmt::Debug, PartialEq, Eq)]
pub struct Frame {
    /// The rule's name, as in [`GrammarRule::NAME`](crate::GrammarRule::NAME).
    pub node: &'static str,
    /// Where it was entered.
    pub pos: usize,
}

/// A parse failure. Its detail depends on the `trace_*` features; read it
/// through [`Error::pos`] and [`Error::traces`].
#[derive(PartialEq, Eq)]
pub struct Error {
    #[cfg(feature = "trace_one_node")]
    #[doc(hidden)]
    pub traces: Vec<Trace>,
    #[cfg(feature = "trace_pos")]
    #[doc(hidden)]
    pub pos: usize,
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

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
                writeln!(f, ", expected:")?;
                for line in lines {
                    writeln!(f, "\t- {line}")?;
                }
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
            Display::fmt(self, f)
        }
    }
}
