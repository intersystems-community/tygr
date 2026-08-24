#![cfg(feature = "trace_one_node")]

use std::str::FromStr;
use tygr::*;

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

fn frame(node: &'static str, pos: usize) -> Frame {
    Frame { node, pos }
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Lit(StringEq!("foo"));

#[test]
fn string_eq_records_literal() {
    let err = Lit::parse("bar").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("Lit", 0)],
                expectation: Expectation::StringEq("foo".to_string()),
            }],
            pos: 0,
        }
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct LitCI(StringEqCI!("foo"));

#[test]
fn string_eq_ci_records_ci_literal() {
    let err = LitCI::parse("bar").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("LitCI", 0)],
                expectation: Expectation::StringEqCI("foo".to_string()),
            }],
            pos: 0,
        }
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Digits(StringOf1<IsDigit>);

#[test]
fn char_class_records_class_name() {
    let err = Digits::parse("abc").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("Digits", 0)],
                expectation: Expectation::CharClass("digit"),
            }],
            pos: 0,
        }
    );
}

#[derive(Debug, PartialEq, Eq, GrammarFromStr)]
struct SmallNumber(u8);

impl GrammarFrom for SmallNumber {
    type Source = StringOf1<IsDigit>;
    fn print_to(&self, buf: &mut String) {
        buf.push_str(&self.0.to_string());
    }
}

impl FromStr for SmallNumber {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u8>().map(SmallNumber)
    }
}

#[test]
fn conversion_records_from_str_error() {
    let err = SmallNumber::parse("999").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![
                Trace {
                    context: vec![],
                    expectation: Expectation::CharClass("digit"),
                },
                Trace {
                    context: vec![],
                    expectation: Expectation::GrammarFrom {
                        from: "999".to_string(),
                        into: "SmallNumber",
                        requirement: "be convertible into SmallNumber",
                        fail: "number too large to fit in target type".to_string(),
                    },
                },
            ],
            pos: 3,
        }
    );
}

#[test]
fn conversion_bnf_shows_the_default_requirement() {
    assert_eq!(
        SmallNumber::bnf_rule(),
        "SmallNumber = ( 'digit' { 'digit' } ) ^1 .\n\n^1: be convertible into SmallNumber"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(validated)]
struct NonZero(StringOf1<IsDigit>);

impl Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[test]
fn validated_records_requirement() {
    let err = NonZero::parse("007").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![
                Trace {
                    context: vec![frame("NonZero", 0)],
                    expectation: Expectation::CharClass("digit"),
                },
                Trace {
                    context: vec![frame("NonZero", 0)],
                    expectation: Expectation::Valid {
                        node: "NonZero",
                        text: "007".to_string(),
                        requirement: "be non-zero",
                    },
                },
            ],
            pos: 3,
        }
    );
}

#[test]
fn display_includes_position_context_and_expectation() {
    let err = NonZero::parse("007").unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at byte 3, expected:\n\t- Char of digit\n\t- NonZero \"007\" must be non-zero\n"
    );
}

#[test]
fn display_shows_each_expectation_kind() {
    let cases = [
        (
            Lit::parse("bar").unwrap_err(),
            "parse error at byte 0, expected:\n\t- Lit\n",
        ),
        (
            LitCI::parse("bar").unwrap_err(),
            "parse error at byte 0, expected:\n\t- LitCI\n",
        ),
        (
            Digits::parse("abc").unwrap_err(),
            "parse error at byte 0, expected:\n\t- Digits\n",
        ),
        (
            SmallNumber::parse("999").unwrap_err(),
            "parse error at byte 3, expected:\n\t- Char of digit\n\t- From 999 to SmallNumber: must be convertible into SmallNumber (number too large to fit in target type)\n",
        ),
        (
            NonZero::parse("007").unwrap_err(),
            "parse error at byte 3, expected:\n\t- Char of digit\n\t- NonZero \"007\" must be non-zero\n",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Wrapper(Lit);

#[test]
#[cfg(not(feature = "trace_all_nodes"))]
fn context_records_nearest_node() {
    let err = Wrapper::parse("bar").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("Lit", 0)],
                expectation: Expectation::StringEq("foo".to_string()),
            }],
            pos: 0,
        }
    );
}

#[test]
#[cfg(feature = "trace_all_nodes")]
fn context_records_full_node_chain() {
    let err = Wrapper::parse("bar").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("Wrapper", 0), frame("Lit", 0)],
                expectation: Expectation::StringEq("foo".to_string()),
            }],
            pos: 0,
        }
    );
}

// 4+ variants trips the derive's FIRST-set dispatch table (see DISPATCH_THRESHOLD
// in tygr-derive), which is what the test below exercises.
#[derive(Grammar, Debug, PartialEq, Eq)]
enum Op {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
    Mul(StringEq!("*")),
    Div(StringEq!("/")),
}

#[test]
fn enum_records_every_variant_when_first_matches_none() {
    let err = Op::parse("x").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: Expectation::StringEq("+".to_string()),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: Expectation::StringEq("-".to_string()),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: Expectation::StringEq("*".to_string()),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: Expectation::StringEq("/".to_string()),
                },
            ],
            pos: 0,
        }
    );
}

// A nullable variant (`Option<..>` always matches empty) inside a 4+-variant
// enum, embedded as a non-last struct field, embedded as a non-last variant of
// another 4+-variant enum — exercises fail_at's OR-chain (nullable variant
// doesn't block the struct's next field) and AND-fold (a required sibling
// variant doesn't block the enum's other siblings) together.
#[derive(Grammar, Debug, PartialEq, Eq)]
enum NullableOp {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
    Mul(StringEq!("*")),
    Maybe(Option<StringEq!("y")>),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct NullableOpThenTail(NullableOp, StringEq!("TAIL"));

#[derive(Grammar, Debug, PartialEq, Eq)]
enum Outermost {
    A(NullableOpThenTail),
    B(StringEq!("b")),
    C(StringEq!("c")),
    D(StringEq!("d")),
}

fn nullable_variant_traces() -> Vec<Trace> {
    vec![
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: Expectation::StringEq("+".to_string()),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: Expectation::StringEq("-".to_string()),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: Expectation::StringEq("*".to_string()),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: Expectation::StringEq("y".to_string()),
        },
        Trace {
            context: vec![frame("NullableOpThenTail", 0)],
            expectation: Expectation::StringEq("TAIL".to_string()),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: Expectation::StringEq("b".to_string()),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: Expectation::StringEq("c".to_string()),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: Expectation::StringEq("d".to_string()),
        },
    ]
}

#[test]
#[cfg(not(feature = "trace_all_nodes"))]
fn nullable_variant_does_not_block_sibling_field_or_sibling_variant() {
    let err = Outermost::parse("x").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: nullable_variant_traces(),
            pos: 0,
        }
    );
}

#[test]
#[cfg(feature = "trace_all_nodes")]
fn nullable_variant_does_not_block_sibling_field_or_sibling_variant() {
    let err = Outermost::parse("x").unwrap_err();
    let mut expected = nullable_variant_traces();
    expected[0].context = vec![
        frame("Outermost", 0),
        frame("NullableOpThenTail", 0),
        frame("NullableOp", 0),
    ];
    expected[1].context = expected[0].context.clone();
    expected[2].context = expected[0].context.clone();
    expected[3].context = expected[0].context.clone();
    expected[4].context = vec![frame("Outermost", 0), frame("NullableOpThenTail", 0)];
    assert_eq!(
        err,
        Error {
            traces: expected,
            pos: 0,
        }
    );
}
