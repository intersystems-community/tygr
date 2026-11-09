//! The state threaded through parsing, and the [`Error`] it traces.

use crate::bnf::escape;
#[cfg(feature = "trace_pos")]
use std::cmp::Ordering;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

/// The stack a parse can use: input nested deeper than that fails to
/// parse, as [`Expectation::TooDeep`], rather than overflow the stack. How
/// deep that is depends on the grammar and the build.
///
/// Every recursive grammar goes through a [`Box`], a [`Vec`], a
/// [`VecSep`](crate::VecSep), a `PhantomData`, a [`Raw`](crate::Raw), a
/// lookahead or a field conversion, as a type can't hold itself; each of
/// these checks the stack. A hand-written grammar that recurses some other
/// way should go through one too.
pub const MAX_STACK: usize = 1 << 20;

/// Parse state threaded through `parse_at`.
#[doc(hidden)]
pub struct State<'a> {
    #[cfg(feature = "trace_pos")]
    history: &'a mut History,
    #[cfg(feature = "trace_one_node")]
    context: Context<'a>,
    /// The lowest stack address parsing can go on at; kept in `history`
    /// when tracing, so the state doesn't grow.
    #[cfg(not(feature = "trace_pos"))]
    floor: usize,
    _marker: PhantomData<&'a ()>,
}

impl<'a> State<'a> {
    fn new(
        #[cfg(feature = "trace_pos")] history: &'a mut History,
        #[cfg(feature = "trace_one_node")] context: Context<'a>,
        #[cfg(not(feature = "trace_pos"))] floor: usize,
    ) -> Self {
        Self {
            #[cfg(feature = "trace_pos")]
            history,
            #[cfg(feature = "trace_one_node")]
            context,
            #[cfg(not(feature = "trace_pos"))]
            floor,
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
            #[cfg(not(feature = "trace_pos"))]
            floor: self.floor,
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
            #[cfg(not(feature = "trace_pos"))]
            floor: self.floor,
            _marker: PhantomData,
        }
    }

    /// Whether there's stack left to go deeper at `pos`; if not, records
    /// that it's too deep. See [`MAX_STACK`].
    #[inline(always)]
    pub(crate) fn room(&mut self, pos: usize) -> bool {
        let room = stack_address() >= self.floor();
        if !room {
            // By value, so the hot path never takes `self`'s address.
            Self::too_deep(self.reborrow(), pos);
        }
        room
    }

    /// The lowest stack address parsing can go on at.
    #[inline(always)]
    fn floor(&self) -> usize {
        #[cfg(feature = "trace_pos")]
        return self.history.floor;
        #[cfg(not(feature = "trace_pos"))]
        return self.floor;
    }

    /// Record that going deeper at `pos` would use too much stack; out of
    /// line when it records, so its callers stay small.
    #[cold]
    #[cfg_attr(feature = "trace_pos", inline(never))]
    fn too_deep(
        #[allow(unused_variables, unused_mut)] mut state: State<'_>,
        #[allow(unused_variables)] pos: usize,
    ) {
        #[cfg(feature = "trace_pos")]
        state.expect(
            pos,
            #[cfg(feature = "trace_one_node")]
            Expectation::TooDeep,
        );
    }

    /// Whether `pos` is the end of `input`; if not, records that it was
    /// expected.
    pub(crate) fn at_end(&mut self, input: &[u8], pos: usize) -> bool {
        let end = pos == input.len();
        #[cfg(feature = "trace_pos")]
        if !end {
            self.expect(
                pos,
                #[cfg(feature = "trace_one_node")]
                Expectation::End,
            );
        }
        end
    }

    /// Record that a negative lookahead ruled out the text at `range`.
    pub(crate) fn not_followed_by(&mut self, #[allow(unused_variables)] range: Range<usize>) {
        #[cfg(feature = "trace_pos")]
        self.expect(
            range.start,
            #[cfg(feature = "trace_one_node")]
            Expectation::NotFollowedBy(range),
        );
    }

    /// Record what was expected at `other_pos`.
    #[cfg(feature = "trace_pos")]
    #[inline]
    pub(crate) fn expect(
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

    /// Record that rule `node` matched `range` but rejected the result,
    /// which must `requirement`.
    #[inline]
    pub fn reject(
        &mut self,
        #[allow(unused_variables)] node: &'static str,
        #[allow(unused_variables)] range: Range<usize>,
        #[allow(unused_variables)] requirement: &'static str,
    ) {
        #[cfg(feature = "trace_pos")]
        self.expect(
            range.end,
            #[cfg(feature = "trace_one_node")]
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
        let mut ignored = History::new(self.floor());
        #[cfg(feature = "trace_pos")]
        {
            ignored.pos = usize::MAX;
        }
        f(State::new(
            #[cfg(feature = "trace_pos")]
            &mut ignored,
            #[cfg(feature = "trace_one_node")]
            self.context,
            #[cfg(not(feature = "trace_pos"))]
            self.floor,
        ))
    }
}

/// About where the stack is now; it grows down.
#[inline(always)]
fn stack_address() -> usize {
    // `pure`: fixed within a frame, so the reads in one frame can merge.
    #[cfg(all(not(miri), target_arch = "aarch64"))]
    {
        let sp: usize;
        // SAFETY: only reads the stack pointer.
        unsafe {
            std::arch::asm!("mov {}, sp", out(reg) sp, options(pure, nomem, nostack, preserves_flags));
        }
        sp
    }
    #[cfg(all(not(miri), target_arch = "x86_64"))]
    {
        let sp: usize;
        // SAFETY: only reads the stack pointer.
        unsafe {
            std::arch::asm!("mov {}, rsp", out(reg) sp, options(pure, nomem, nostack, preserves_flags));
        }
        sp
    }
    #[cfg(any(miri, not(any(target_arch = "aarch64", target_arch = "x86_64"))))]
    {
        let here = 0u8;
        (&raw const here).addr()
    }
}

/// Run a parse from a fresh state, turning `None` into the traced `Error`.
pub(crate) fn run<R>(parse: impl FnOnce(State<'_>) -> Option<R>) -> Result<R, Error> {
    // Miri's addresses aren't a stack's.
    let floor = if cfg!(miri) {
        0
    } else {
        stack_address().saturating_sub(MAX_STACK)
    };
    #[cfg(feature = "trace_pos")]
    let mut history = History::new(floor);
    let result = parse(State::new(
        #[cfg(feature = "trace_pos")]
        &mut history,
        #[cfg(feature = "trace_one_node")]
        Context::default(),
        #[cfg(not(feature = "trace_pos"))]
        floor,
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
    /// The lowest stack address parsing can go on at.
    floor: usize,
}

/// A recorded trace, with its context kept cheap until the parse fails.
#[cfg(feature = "trace_one_node")]
#[derive(Debug)]
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
    fn new(floor: usize) -> Self {
        History {
            #[cfg(feature = "trace_one_node")]
            traces: Vec::new(),
            pos: 0,
            floor,
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
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expectation {
    /// This literal (`StringEq!` / `StringEqCI!`) didn't match.
    Token(&'static [u8]),
    /// No character satisfying this [`CharClass`](crate::CharClass) was found.
    CharClass(&'static str),
    /// A rule matched `range` but rejected the result: a
    /// `#[tygr(validated)]` check or a field conversion failed.
    Rejected {
        /// The rule's name.
        node: &'static str,
        /// The bytes it matched.
        range: Range<usize>,
        /// What the result must do: a
        /// [`Validate::REQUIREMENT`](crate::Validate::REQUIREMENT), or
        /// `"be convertible"`.
        requirement: &'static str,
    },
    /// Anything but the text at this range, which a
    /// [`NotFollowedBy`](crate::NotFollowedBy) rules out.
    NotFollowedBy(Range<usize>),
    /// The end of input: the grammar matched only part of it.
    End,
    /// Shallower nesting: going deeper would use more than [`MAX_STACK`].
    TooDeep,
}

impl fmt::Display for Expectation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expectation::Token(t) => {
                write!(f, "\"{}\"", escape(&String::from_utf8_lossy(t), '"'))
            }
            Expectation::CharClass(c) => write!(f, "Char of {c}"),
            Expectation::Rejected {
                node,
                range,
                requirement,
            } => write!(f, "{node} at {range:?} must {requirement}"),
            Expectation::NotFollowedBy(range) => write!(f, "anything but {range:?}"),
            Expectation::End => write!(f, "end of input"),
            Expectation::TooDeep => write!(f, "shallower nesting"),
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
                        // A rule that starts here stands for what it expected,
                        // unless it was nested too deeply to enter.
                        match context.last() {
                            Some(frame)
                                if frame.pos == self.pos
                                    && *expectation != Expectation::TooDeep =>
                            {
                                frame.node.to_string()
                            }
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
