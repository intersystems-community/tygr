//! Variants that start with the same fields parse them once.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use tygr::bnf::{Expr, Rules};
use tygr::*;

static PARSES: AtomicUsize = AtomicUsize::new(0);
static SERIAL: Mutex<()> = Mutex::new(());

/// `a`, counting its parses.
#[derive(Debug)]
struct A;

impl Parse for A {
    type First = <StringEq!("a") as Parse>::First;

    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        PARSES.fetch_add(1, Relaxed);
        let ((), end) = <StringEq!("a")>::parse_at::<success::Unit, L>(input, pos, state)?;
        Some((S::output(|| A), end))
    }

    fn to_bnf() -> Expr {
        <StringEq!("a")>::to_bnf()
    }

    fn visit_rules(_rules: &mut Rules) {}
}

/// `f`'s result and how many times it parsed an `A`; one test at a time.
fn parses<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = PARSES.load(Relaxed);
    let result = f();
    (result, PARSES.load(Relaxed) - before)
}

/// Too few variants for a dispatch table: tried in order at every level.
#[derive(Parse, Debug)]
enum Three {
    Ax(A, StringEq!("x")),
    Ay(A, StringEq!("y")),
    Az(A, StringEq!("z")),
}

/// Enough for a table below `Max`; `Axq` and `Axr` share two fields.
#[derive(Parse, Debug)]
enum Five {
    Axq(A, StringEq!("x"), StringEq!("q")),
    Ay(A, StringEq!("y")),
    Axr(A, StringEq!("x"), StringEq!("r")),
    B(StringEq!("b")),
    Az(A, StringEq!("z")),
}

#[test]
fn a_shared_prefix_parses_once() {
    assert_eq!(parses(|| Three::check("az")), (Ok(()), 1));
    assert_eq!(
        parses(|| Five::parse_with::<success::Unit, level::Min>("axr")).1,
        1
    );
    assert_eq!(
        parses(|| Five::parse_with::<success::Unit, level::Pos>("axr")).1,
        1
    );
    assert_eq!(parses(|| Five::check("axr")), (Ok(()), 1));
    assert!(matches!(
        parses(|| Five::parse("axq")),
        (Ok(Five::Axq(A, _, _)), 1)
    ));
}

#[test]
fn a_shared_field_fails_once_for_every_variant_starting_with_it() {
    assert_eq!(
        parses(|| Three::parse_with::<success::Unit, level::Min>("q")).1,
        1
    );
    // Below `Max` the table misses on `q`; `Max` then tries every variant.
    let (result, parses) = parses(|| Five::check("qq"));
    assert_eq!(parses, 1);
    let mut expected: Vec<String> = result
        .unwrap_err()
        .traces()
        .iter()
        .map(|trace| trace.expectation.to_string())
        .collect();
    expected.sort();
    assert_eq!(expected, [r#""a""#, r#""b""#]);
}

/// Nullable shared fields: on `d` the table skips `Ac`, so `Ad` is the first
/// variant through the shared nodes and parses them itself.
#[derive(Parse, Debug)]
enum Nullable {
    Ac(Option<StringEq!("+")>, Option<A>, StringEq!("c")),
    Ad(Option<StringEq!("+")>, Option<A>, StringEq!("d")),
    B(StringEq!("b")),
    E(StringEq!("e")),
}

#[test]
fn the_first_variant_to_run_parses_the_shared_fields() {
    let result = parses(|| Nullable::parse_with::<success::Value, level::Min>("+ad"));
    assert!(matches!(result, (Ok(Nullable::Ad(Some(_), Some(A), _)), 1)));
}

#[derive(Parse, Debug)]
#[tygr(validated)]
enum Validated {
    Ax(A, StringEq!("x")),
    Ay(A, StringEq!("y")),
}

impl Validate for Validated {
    const REQUIREMENT: &'static str = "be anything";
    fn validate(&self) -> bool {
        true
    }
}

#[test]
fn a_validated_enum_shares_too() {
    assert_eq!(parses(|| Validated::check("ay")), (Ok(()), 1));
}
