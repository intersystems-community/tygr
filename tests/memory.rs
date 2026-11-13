//! A failed parse keeps one copy of each thing it expected, however often a
//! backtracking grammar expects it.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use tygr::bnf::{Expr, Rules};
use tygr::*;

/// Counts live heap bytes and their peak.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(live, Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Rejects every char with a message built from it, only in the pass that
/// records it.
#[derive(Debug)]
struct Never;

impl Parse for Never {
    type First = <char as Parse>::First;

    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let mut state = state.node("Never", pos);
        let (ch, end) = char::build_at(input, pos, state.reborrow())?;
        let requirement = if M::RECORD {
            format!("not be {ch:?}")
        } else {
            String::new()
        };
        state.reject("Never", pos..end, requirement);
        None
    }

    fn fail_at<M: Mode>(pos: usize, mut state: State<M>) -> bool {
        char::fail_at(pos, state.node("Never", pos))
    }

    fn to_bnf() -> Expr {
        Expr::RuleRef("Never".into())
    }

    fn visit_rules(rules: &mut Rules) {
        rules.insert::<Self>("Never", || Expr::side_condition(char::to_bnf(), "not be"));
    }
}

/// `S = "a" S "b" | "a" S "c" | Never`: on `a…a7`, `Never` rejects the `7`
/// once per path through the recursion, 2^n times for n `a`s.
#[derive(Parse, Debug)]
enum Exponential {
    B(StringEq!("a"), Box<Exponential>, StringEq!("b")),
    C(StringEq!("a"), Box<Exponential>, StringEq!("c")),
    Last(Never),
}

#[test]
fn a_repeated_owned_requirement_is_kept_once() {
    let input = format!("{}7", "a".repeat(16));
    let before = LIVE.load(Relaxed);
    PEAK.store(before, Relaxed);
    let err = Exponential::parse(&input).unwrap_err();
    let peak = PEAK.load(Relaxed) - before;
    assert_eq!(
        err.to_string(),
        "parse error at byte 17, expected:\n\t- Never at 16..17 must not be '7'"
    );
    // 65,536 rejections of one message: 2.8 MB when each was kept (#63).
    assert!(peak < 64 * 1024, "peak heap during the parse: {peak} bytes");
}
