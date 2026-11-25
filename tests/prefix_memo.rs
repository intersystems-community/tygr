//! Variants that start with the same fields parse them once: a later variant
//! finds what an earlier one parsed, or that it failed, and goes on from
//! there.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use tygr::bnf::{Expr, Rules};
use tygr::*;

static PARSES: AtomicUsize = AtomicUsize::new(0);

/// The tests share the counter, so they run one at a time.
static SERIAL: Mutex<()> = Mutex::new(());

/// `a`, counting each attempt to parse it.
#[derive(Debug, PartialEq)]
struct Counted;

impl Parse for Counted {
    type First = <StringEq!("a") as Parse>::First;

    fn parse_at<S: Success, L: Level>(
        input: &str,
        pos: usize,
        state: State<L>,
    ) -> Option<(S::Output<Self>, usize)> {
        PARSES.fetch_add(1, Relaxed);
        let ((), end) = <StringEq!("a")>::parse_at::<success::Unit, L>(input, pos, state)?;
        Some((S::output(|| Counted), end))
    }

    fn to_bnf() -> Expr {
        <StringEq!("a")>::to_bnf()
    }

    fn visit_rules(_rules: &mut Rules) {}
}

fn parses_during<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let before = PARSES.load(Relaxed);
    let result = f();
    (result, PARSES.load(Relaxed) - before)
}

/// Three variants: tried in order at every level.
#[derive(Parse, Debug)]
enum Three {
    Ax(Counted, StringEq!("x")),
    Ay(Counted, StringEq!("y")),
    Az(Counted, StringEq!("z")),
}

/// Five variants: a dispatch table below `Max`. `Axq` and `Axr` share two
/// fields; `Ay`, `Az` and they share the first.
#[derive(Parse, Debug)]
enum Five {
    Axq(Counted, StringEq!("x"), StringEq!("q")),
    Ay(Counted, StringEq!("y")),
    Axr(Counted, StringEq!("x"), StringEq!("r")),
    B(StringEq!("b")),
    Az(Counted, StringEq!("z")),
}

#[test]
fn a_shared_first_field_parses_once() {
    let (result, parses) = parses_during(|| Three::parse("az"));
    assert!(matches!(result, Ok(Three::Az(Counted, _))));
    assert_eq!(parses, 1);
    let (result, parses) = parses_during(|| Five::parse_with::<success::Value, level::Min>("az"));
    assert!(matches!(result, Ok(Five::Az(Counted, _))));
    assert_eq!(parses, 1);
}

#[test]
fn a_shared_prefix_of_two_fields_parses_once() {
    let checks: [fn(&str) -> bool; 3] = [
        |s| Five::parse_with::<success::Unit, level::Min>(s).is_ok(),
        |s| Five::parse_with::<success::Unit, level::Pos>(s).is_ok(),
        |s| Five::check(s).is_ok(),
    ];
    for check in checks {
        let (ok, parses) = parses_during(|| check("axr"));
        assert!(ok);
        assert_eq!(parses, 1);
    }
}

#[test]
fn a_shared_field_that_failed_fails_the_later_variants_without_parsing() {
    let (result, parses) = parses_during(|| Three::parse_with::<success::Unit, level::Min>("q"));
    assert!(result.is_err());
    assert_eq!(parses, 1);
    // At `Max`, the first pass misses on the table; the second tries every
    // variant in order, and `Counted` fails once for all four that start with it.
    let (result, parses) = parses_during(|| Five::check("qq"));
    let err = result.unwrap_err();
    assert_eq!(parses, 1);
    let mut expected: Vec<String> = err
        .traces()
        .iter()
        .map(|t| t.expectation.to_string())
        .collect();
    expected.sort();
    assert_eq!(expected, [r#""a""#, r#""b""#]);
}

#[test]
fn the_succeeding_variant_takes_the_shared_values() {
    let _serial = SERIAL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let value = Five::parse("ay").unwrap();
    assert!(matches!(value, Five::Ay(Counted, _)));
    assert_eq!(Five::parse("axq").unwrap().print_len(), 3);
}

impl Five {
    fn print_len(&self) -> usize {
        match self {
            Five::Axq(..) | Five::Axr(..) => 3,
            Five::Ay(..) | Five::Az(..) => 2,
            Five::B(_) => 1,
        }
    }
}

/// Nullable shared fields: on `d`, the table skips `Ac` for `Ad`, the first
/// variant to run through the shared nodes, which must parse them itself.
#[derive(Parse, Debug)]
enum Nullable {
    Ac(Option<StringEq!("+")>, Option<Counted>, StringEq!("c")),
    Ad(Option<StringEq!("+")>, Option<Counted>, StringEq!("d")),
    B(StringEq!("b")),
    E(StringEq!("e")),
}

#[test]
fn a_skipped_first_variant_leaves_the_slots_untried() {
    let (result, parses) =
        parses_during(|| Nullable::parse_with::<success::Value, level::Min>("d"));
    assert!(matches!(result, Ok(Nullable::Ad(None, None, _))));
    assert_eq!(parses, 1);
    let (result, parses) =
        parses_during(|| Nullable::parse_with::<success::Value, level::Min>("+ad"));
    assert!(matches!(
        result,
        Ok(Nullable::Ad(Some(_), Some(Counted), _))
    ));
    assert_eq!(parses, 1);
}

/// A validated enum parses its fields as values whatever the caller asked.
#[derive(Parse, Debug)]
#[tygr(validated)]
enum Validated {
    Ax(Counted, StringEq!("x")),
    Ay(Counted, StringEq!("y")),
}

impl Validate for Validated {
    const REQUIREMENT: &'static str = "be anything";
    fn validate(&self) -> bool {
        true
    }
}

#[test]
fn a_validated_enum_shares_prefixes_too() {
    let (result, parses) = parses_during(|| Validated::check("ay"));
    assert_eq!(result, Ok(()));
    assert_eq!(parses, 1);
}
