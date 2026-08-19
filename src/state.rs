#[cfg(feature = "trace_one_node")]
use core::str;
#[cfg(feature = "trace_pos")]
use std::cmp::Ordering;
use std::{fmt::Display, marker::PhantomData};

/// Threaded parse state: the error `History` and the call-stack `Context`.
/// Both are behind feature flags, so `State` is a ZST when nothing is traced.
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
    attempts: Vec<Attempt>,
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
        let attempts = {
            let mut attempts = self.attempts;
            for attempt in &mut attempts {
                attempt.context.reverse()
            }
            attempts
        };
        Error {
            #[cfg(feature = "trace_one_node")]
            attempts,
            #[cfg(feature = "trace_pos")]
            pos: self.pos,
        }
    }
}

#[derive(std::fmt::Debug)]
pub enum Expectation {
    StringEq(String),
    StringEqCI(String),
    CharClass(&'static str),
    Filtered { pos: usize, be_valid: &'static str },
    Conversion(String),
}

#[cfg(feature = "trace_one_node")]
impl Expectation {
    pub fn pos(&self) -> Option<usize> {
        if let Expectation::Filtered { pos, .. } = self {
            Some(*pos)
        } else {
            None
        }
    }
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
            Expectation::StringEq(s) => write!(f, "\"{}\"", escape_string(s)),
            Expectation::StringEqCI(s) => {
                write!(f, "\"{}\"i", escape_string(s))
            }
            Expectation::CharClass(c) => write!(f, "{c}"),
            Expectation::Filtered { be_valid, .. } => {
                write!(f, "The proceeding unit must {be_valid}")
            }
            Expectation::Conversion(msg) => write!(f, "{msg}"),
        }
    }
}

#[derive(std::fmt::Debug)]
pub struct Attempt {
    pub context: Vec<(&'static str, usize)>,
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
        let attempt = || Attempt {
            context: context.to_vec(),
            expectation,
        };
        match self.pos.cmp(&other_pos) {
            Ordering::Equal => {
                #[cfg(feature = "trace_one_node")]
                {
                    self.attempts.push(attempt())
                }
            }
            Ordering::Less => {
                self.pos = other_pos;
                #[cfg(feature = "trace_one_node")]
                {
                    self.attempts = vec![attempt()];
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
    fn to_vec(self) -> Vec<(&'static str, usize)> {
        #[cfg(feature = "trace_all_nodes")]
        {
            let mut result = vec![];
            let mut cursor = self;
            while let Some(node) = cursor.0 {
                result.push((node.first.node, node.first.pos));
                cursor = *node.remaining;
            }
            return result;
        }

        #[cfg(not(feature = "trace_all_nodes"))]
        {
            self.0
                .map(|node| vec![(node.first.node, node.first.pos)])
                .unwrap_or_default()
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

#[cfg(feature = "trace_one_node")]
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Frame {
    node: &'static str,
    pos: usize,
}

pub struct Error {
    #[cfg(feature = "trace_one_node")]
    pub attempts: Vec<Attempt>,
    #[cfg(feature = "trace_pos")]
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
    pub fn attempts(&self) -> &[Attempt] {
        #[cfg(feature = "trace_one_node")]
        {
            &self.attempts
        }
        #[cfg(not(feature = "trace_one_node"))]
        {
            &[]
        }
    }

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
        #[cfg(feature = "trace_one_node")]
        {
            let attempts: std::collections::HashSet<_> = self
                .attempts
                .iter()
                .map(
                    |Attempt {
                         context,
                         expectation,
                     }| {
                        if let Some((node, pos)) = context.last() {
                            if *pos == self.pos {
                                node.to_string()
                            } else {
                                format!("{expectation}")
                            }
                        } else {
                            format!("{expectation}")
                        }
                    },
                )
                .collect();
            let mut attempts: Vec<String> = attempts.into_iter().collect();
            attempts.sort();
            for attempt in attempts {
                writeln!(f, "\t-{attempt}")?;
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
            for attempt in &self.attempts {
                writeln!(
                    f,
                    "\t- {}!{}",
                    attempt
                        .context
                        .iter()
                        .map(|(node, pos)| { format!("{node}@:{pos}") })
                        .collect::<Vec<String>>()
                        .join("/"),
                    attempt.expectation
                )?;
            }
        }
        Ok(())
    }
}
