#[cfg(feature = "trace_one_node")]
use core::str;
#[cfg(feature = "trace_pos")]
use std::cmp::Ordering;
use std::{fmt::Display, marker::PhantomData};

/// Threaded parse state: the error `History` and the call-stack `Context`.
/// Both are behind feature flags, so `State` is a ZST when nothing is traced.
#[doc(hidden)]
pub struct State<'a> {
    #[cfg(feature = "trace_pos")]
    history: &'a mut History,
    #[cfg(feature = "trace_one_node")]
    context: Context<'a>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> State<'a> {
    pub(crate) fn new(
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

    /// Reborrow for a nested call. `State` isn't `Copy` because it holds `&mut`.
    pub fn reborrow(&mut self) -> State<'_> {
        State {
            #[cfg(feature = "trace_pos")]
            history: &mut *self.history,
            #[cfg(feature = "trace_one_node")]
            context: self.context,
            _marker: PhantomData,
        }
    }

    /// Push a rule frame onto the context path (a no-op unless `context` is on).
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

    /// Record a failed expectation at `other_pos` (a no-op unless tracing).
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

    /// Probe with a throwaway history so a match or miss during lookahead
    /// doesn't pollute the real error trace.
    pub fn probe<R>(&mut self, f: impl FnOnce(State<'_>) -> R) -> R {
        #[cfg(feature = "trace_pos")]
        let mut scratch = History::new();
        f(State::new(
            #[cfg(feature = "trace_pos")]
            &mut scratch,
            #[cfg(feature = "trace_one_node")]
            self.context,
        ))
    }
}

#[cfg(feature = "trace_pos")]
#[derive(std::fmt::Debug, Default)]
pub(crate) struct History {
    #[cfg(feature = "trace_one_node")]
    traces: Vec<Trace>,
    #[cfg(feature = "trace_pos")]
    pos: usize,
}

#[cfg(feature = "trace_pos")]
impl History {
    pub(crate) fn new() -> Self {
        History::default()
    }

    pub(crate) fn into_error(self) -> Error {
        #[cfg(feature = "trace_one_node")]
        let traces = {
            let mut traces = self.traces;
            for trace in &mut traces {
                trace.context.reverse()
            }
            traces
        };
        Error {
            #[cfg(feature = "trace_one_node")]
            traces,
            #[cfg(feature = "trace_pos")]
            pos: self.pos,
        }
    }
}

/// One candidate for what would have matched at a [`Trace`]'s recorded position.
#[derive(std::fmt::Debug, PartialEq, Eq)]
pub enum Expectation {
    /// A case-sensitive literal (from `StringEq!`) didn't match.
    StringEq(String, String),
    /// A case-insensitive literal (from `StringEqCI!`) didn't match.
    StringEqCI(String, String),
    /// No character satisfying this [`CharClass`](crate::CharClass) was found.
    CharClass(&'static str),
    /// A `#[grammar(validated)]` type parsed successfully but
    /// [`Validate::validate`](crate::Validate::validate) rejected it.
    Valid {
        /// The rejected type's own [`GrammarRule::NAME`](crate::GrammarRule::NAME).
        node: &'static str,
        /// The raw text that was parsed and then rejected.
        text: String,
        /// The rejected type's [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT).
        requirement: &'static str,
    },
    /// A `GrammarFromStr`/`GrammarFromOther`/`GrammarTryFromOther`-derived
    /// type matched its source grammar but failed to convert.
    GrammarFrom {
        /// The raw text that matched the source grammar but failed to convert.
        from: String,
        /// The target type's own [`GrammarRule::NAME`](crate::GrammarRule::NAME).
        into: &'static str,
        /// The conversion error's `Display` text.
        fail: String,
    },
}

#[cfg(feature = "trace_one_node")]
fn escape_string(s: &str) -> String {
    s.chars()
        .map(|char| match char {
            '\t' => "\\t".to_string(),
            '\n' => "\\n".to_string(),
            '\r' => "\\r".to_string(),
            '"' => "\\\"".to_string(),
            char => char.to_string(),
        })
        .collect()
}

#[cfg(feature = "trace_one_node")]
impl Display for Expectation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expectation::StringEq(partial, full) => {
                // `partial` is always a suffix of `full`; equal length means
                // no prefix was stripped, i.e. they're the same string.
                if partial.len() == full.len() {
                    write!(f, "\"{}\"", escape_string(full))
                } else {
                    write!(
                        f,
                        "\"{}\" of \"{}\"",
                        escape_string(partial),
                        escape_string(full)
                    )
                }
            }
            Expectation::StringEqCI(partial, full) => {
                if partial.len() == full.len() {
                    write!(f, "\"{}\" (case-insensitive)", escape_string(full))
                } else {
                    write!(
                        f,
                        "\"{}\" of \"{}\" (case-insensitive)",
                        escape_string(partial),
                        escape_string(full)
                    )
                }
            }
            Expectation::CharClass(c) => write!(f, "Char of {c}"),
            Expectation::Valid {
                node,
                text,
                requirement,
            } => {
                write!(f, "{node} \"{}\" must {requirement}", escape_string(text))
            }
            Expectation::GrammarFrom { from, into, fail } => {
                write!(f, "From {from} to {into}: {fail}")
            }
        }
    }
}

/// One recorded parse attempt at [`Error::pos`] — a rule call chain and what
/// it expected to find there. Multiple `Trace`s at the same position are
/// candidates: any one of them matching would have let the parse continue.
#[derive(std::fmt::Debug, PartialEq, Eq)]
pub struct Trace {
    /// The nearest enclosing named rule(s), outermost first. Only ever more
    /// than one element with `trace_all_nodes` enabled.
    pub context: Vec<Frame>,
    /// What was expected at this position.
    pub expectation: Expectation,
}

#[cfg(feature = "trace_pos")]
impl History {
    pub fn expect(
        &mut self,
        #[cfg(feature = "trace_one_node")] context: Context<'_>,
        other_pos: usize,
        #[cfg(feature = "trace_one_node")] expectation: Expectation,
    ) {
        #[cfg(feature = "trace_one_node")]
        let trace = || Trace {
            context: context.to_vec(),
            expectation,
        };
        match self.pos.cmp(&other_pos) {
            Ordering::Equal => {
                #[cfg(feature = "trace_one_node")]
                {
                    self.traces.push(trace())
                }
            }
            Ordering::Less => {
                self.pos = other_pos;
                #[cfg(feature = "trace_one_node")]
                {
                    self.traces = vec![trace()];
                }
            }
            Ordering::Greater => (),
        }
    }
}

/// Immutable call-stack path threaded by value; a ZST unless `context` is on.
#[doc(hidden)]
#[derive(Clone, Copy, Default)]
pub struct Context<'a>(
    #[cfg(feature = "trace_one_node")] Option<ContextNode<'a>>,
    PhantomData<&'a ()>,
);

#[cfg(feature = "trace_one_node")]
#[derive(Clone, Copy)]
struct ContextNode<'a> {
    first: Frame,
    #[cfg(feature = "trace_one_node")]
    _marker: PhantomData<&'a ()>,
    #[cfg(feature = "trace_all_nodes")]
    remaining: &'a Context<'a>,
}

impl<'a> Context<'a> {
    #[cfg(feature = "trace_one_node")]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "trace_one_node")]
    fn to_vec(self) -> Vec<Frame> {
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
            self.0.map(|node| vec![node.first]).unwrap_or_default()
        }
    }

    #[cfg(feature = "trace_one_node")]
    fn node(&'a self, node: &'static str, pos: usize) -> Context<'a> {
        Context(
            Some(ContextNode {
                first: Frame { node, pos },
                #[cfg(feature = "trace_one_node")]
                _marker: PhantomData,
                #[cfg(feature = "trace_all_nodes")]
                remaining: self,
            }),
            PhantomData,
        )
    }
}

/// One named rule call in a [`Trace`]'s [`context`](Trace::context).
#[derive(Clone, Copy, std::fmt::Debug, PartialEq, Eq)]
pub struct Frame {
    /// The rule's name, as in [`GrammarRule::NAME`](crate::GrammarRule::NAME).
    pub node: &'static str,
    /// The position this rule was entered at.
    pub pos: usize,
}

/// A parse failure. Carries no detail unless the relevant `trace_*` feature
/// is enabled (see the crate-level feature flag table); use [`Error::pos`]
/// and [`Error::traces`] to inspect it rather than the fields directly, since
/// those degrade gracefully across feature configurations.
#[derive(PartialEq, Eq)]
pub struct Error {
    #[cfg(feature = "trace_one_node")]
    #[doc(hidden)]
    pub traces: Vec<Trace>,
    #[cfg(feature = "trace_pos")]
    #[doc(hidden)]
    pub pos: usize,
}

/// Build the parse `Error`. When `history` is off, `Error` carries no data.
pub(crate) fn make_error(#[cfg(feature = "trace_pos")] history: History) -> Error {
    #[cfg(feature = "trace_pos")]
    {
        history.into_error()
    }
    #[cfg(not(feature = "trace_pos"))]
    {
        Error {}
    }
}

impl Error {
    /// The candidate expectations recorded at the deepest position reached.
    /// Empty unless `trace_one_node` is enabled.
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

    /// The deepest byte offset the parser reached before giving up. `0`
    /// unless `trace_pos` is enabled.
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
    #[allow(unused_variables)]
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
                        if let Some(frame) = context.last()
                            && frame.pos == self.pos
                        {
                            frame.node.to_string()
                        } else {
                            format!("{expectation}")
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
    #[allow(unused_variables)]
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
        }
        Ok(())
    }
}
