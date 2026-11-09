//! Parsing uses at most `MAX_STACK` of stack, so deeply nested input fails
//! instead of overflowing it.

use tygr::*;

#[path = "../examples/common/json.rs"]
mod json;

#[derive(Grammar, Debug)]
struct Nest(StringEq!("("), Option<Box<Nest>>, StringEq!(")"));

fn nested(depth: usize) -> String {
    "(".repeat(depth) + &")".repeat(depth)
}

fn array(depth: usize) -> String {
    "[".repeat(depth) + &"]".repeat(depth)
}

#[test]
fn moderate_nesting_parses() {
    assert!(Nest::parse(&nested(200)).is_ok());
    assert!(json::Json::parse(&array(50)).is_ok());
}

#[test]
fn deep_nesting_fails_without_overflowing() {
    assert!(Nest::parse(&nested(1_000_000)).is_err());
    assert!(Nest::check(&nested(1_000_000)).is_err());
    assert!(json::Json::parse(&array(100_000)).is_err());
    assert!(json::Json::check(&array(100_000)).is_err());
}
