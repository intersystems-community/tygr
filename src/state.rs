#[cfg(feature = "trace_attempts")]
use core::str;
#[cfg(feature = "trace_any")]
use std::cmp::Ordering;
use std::{fmt::Display, marker::PhantomData};

/// Threaded parse state: the error `History` and the call-stack `Context`.
/// Both are behind feature flags, so `State` is a ZST when nothing is traced.
pub struct State<'a> {
    #[cfg(feature = "history")]
    history: &'a mut History,
    #[cfg(feature = "context")]
    context: Context<'a>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> State<'a> {
    pub(crate) fn new(
        #[cfg(feature = "history")] history: &'a mut History,
        #[cfg(feature = "context")] context: Context<'a>,
    ) -> Self {
        Self {
            #[cfg(feature = "history")]
            history,
            #[cfg(feature = "context")]
            context,
            _marker: PhantomData,
        }
    }

    /// Reborrow for a nested call. `State` isn't `Copy` because it holds `&mut`.
    pub fn reborrow(&mut self) -> State<'_> {
        State {
            #[cfg(feature = "history")]
            history: &mut *self.history,
            #[cfg(feature = "context")]
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
            #[cfg(feature = "history")]
            history: &mut *self.history,
            #[cfg(feature = "context")]
            context: self.context.node(node, pos),
            _marker: PhantomData,
        }
    }

    /// Record a failed expectation at `other_pos` (a no-op unless tracing).
    #[cfg(feature = "trace_any")]
    #[inline]
    pub fn expect(
        &mut self,
        other_pos: usize,
        #[cfg(feature = "trace_attempts")] expectation: Expectation,
    ) {
        self.history.expect(
            #[cfg(feature = "trace_attempts")]
            self.context,
            other_pos,
            #[cfg(feature = "trace_attempts")]
            expectation,
        );
    }

    /// Probe with a throwaway history so a match or miss during lookahead
    /// doesn't pollute the real error trace.
    pub fn probe<R>(&mut self, f: impl FnOnce(State<'_>) -> R) -> R {
        #[cfg(feature = "history")]
        let mut scratch = History::new();
        f(State::new(
            #[cfg(feature = "history")]
            &mut scratch,
            #[cfg(feature = "context")]
            self.context,
        ))
    }
}

#[cfg(feature = "history")]
#[derive(std::fmt::Debug, Default)]
pub(crate) struct History {
    #[cfg(feature = "trace_attempts")]
    attempts: Vec<Attempt>,
    #[cfg(feature = "trace_pos")]
    pos: usize,
}

#[cfg(feature = "history")]
impl History {
    pub(crate) fn new() -> Self {
        History::default()
    }

    pub(crate) fn into_error(self) -> Error {
        #[cfg(feature = "trace_attempts")]
        let attempts = {
            let mut attempts = self.attempts;
            for attempt in &mut attempts {
                attempt.context.reverse()
            }
            attempts
        };
        Error {
            #[cfg(feature = "trace_attempts")]
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

#[cfg(feature = "trace_attempts")]
impl Expectation {
    pub fn pos(&self) -> Option<usize> {
        if let Expectation::Filtered { pos, .. } = self {
            Some(*pos)
        } else {
            None
        }
    }
}

#[cfg(feature = "trace_attempts")]
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

#[cfg(feature = "trace_attempts")]
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

#[cfg(feature = "trace_any")]
impl History {
    pub fn expect(
        &mut self,
        #[cfg(feature = "trace_attempts")] context: Context<'_>,
        other_pos: usize,
        #[cfg(feature = "trace_attempts")] expectation: Expectation,
    ) {
        #[cfg(feature = "trace_attempts")]
        let attempt = || Attempt {
            context: context.to_vec(),
            expectation,
        };
        match self.pos.cmp(&other_pos) {
            Ordering::Equal => {
                #[cfg(feature = "trace_attempts")]
                {
                    self.attempts.push(attempt())
                }
            }
            Ordering::Less => {
                self.pos = other_pos;
                #[cfg(feature = "trace_attempts")]
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
    #[cfg(feature = "context")] Option<ContextNode<'a>>,
    PhantomData<&'a ()>,
);

#[cfg(feature = "context")]
#[derive(Clone, Copy)]
struct ContextNode<'a> {
    first: Frame,
    remaining: &'a Context<'a>,
}

impl<'a> Context<'a> {
    #[cfg(feature = "context")]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "trace_attempts")]
    fn to_vec(self) -> Vec<(&'static str, usize)> {
        let mut result = vec![];
        let mut cursor = self;
        while let Some(node) = cursor.0 {
            result.push((node.first.node, node.first.pos));
            cursor = *node.remaining;
        }
        result
    }

    #[cfg(feature = "context")]
    fn node(&'a self, node: &'static str, pos: usize) -> Context<'a> {
        Context(
            Some(ContextNode {
                first: Frame { node, pos },
                remaining: self,
            }),
            PhantomData,
        )
    }
}

#[cfg(feature = "context")]
#[doc(hidden)]
#[derive(Clone, Copy)]
pub struct Frame {
    node: &'static str,
    pos: usize,
}

pub struct Error {
    #[cfg(feature = "trace_attempts")]
    pub attempts: Vec<Attempt>,
    #[cfg(feature = "trace_pos")]
    pub pos: usize,
}

/// Build the parse `Error`. When `history` is off, `Error` carries no data.
pub(crate) fn make_error(#[cfg(feature = "history")] history: History) -> Error {
    #[cfg(feature = "history")]
    {
        history.into_error()
    }
    #[cfg(not(feature = "history"))]
    {
        Error {}
    }
}

impl Error {
    pub fn attempts(&self) -> &[Attempt] {
        #[cfg(feature = "trace_attempts")]
        {
            &self.attempts
        }
        #[cfg(not(feature = "trace_attempts"))]
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
        #[cfg(feature = "trace_attempts")]
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
        #[cfg(feature = "trace_attempts")]
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
