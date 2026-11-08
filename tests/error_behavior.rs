#![cfg(feature = "trace_one_node")]

use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

fn token(text: &'static str) -> Expectation {
    Expectation::Token(text.as_bytes())
}

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
                expectation: token("foo"),
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
                expectation: token("foo"),
            }],
            pos: 0,
        }
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Arrow(StringEq!("->"));

#[test]
fn string_eq_partial_match_records_at_its_start() {
    let err = Arrow::parse("-x").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![Trace {
                context: vec![frame("Arrow", 0)],
                expectation: token("->"),
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

#[derive(Grammar, Debug, PartialEq, Eq)]
struct SmallNumber(#[tygr(from_str = StringOf1<IsDigit>)] u8);

#[test]
fn conversion_records_from_str_error() {
    let err = SmallNumber::parse("999").unwrap_err();
    assert_eq!(
        err,
        Error {
            traces: vec![
                Trace {
                    context: vec![frame("SmallNumber", 0)],
                    expectation: Expectation::CharClass("digit"),
                },
                Trace {
                    context: vec![frame("SmallNumber", 0)],
                    expectation: Expectation::Rejected {
                        node: "SmallNumber",
                        range: 0..3,
                        requirement: "be convertible",
                    },
                },
            ],
            pos: 3,
        }
    );
}

#[test]
fn conversion_bnf_shows_the_side_condition() {
    assert_eq!(
        rule::<SmallNumber>(),
        "SmallNumber = ( 'digit' { 'digit' } ) ^1 .\n\n^1: be convertible"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(validated)]
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
                    expectation: Expectation::Rejected {
                        node: "NonZero",
                        range: 0..3,
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
        "parse error at byte 3, expected:\n\t- Char of digit\n\t- NonZero at 0..3 must be non-zero\n"
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
            "parse error at byte 3, expected:\n\t- Char of digit\n\t- SmallNumber at 0..3 must be convertible\n",
        ),
        (
            NonZero::parse("007").unwrap_err(),
            "parse error at byte 3, expected:\n\t- Char of digit\n\t- NonZero at 0..3 must be non-zero\n",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn display_escapes_tokens() {
    assert_eq!(token("\\\"\n").to_string(), r#""\\\"\n""#);
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
                expectation: token("foo"),
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
                expectation: token("foo"),
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
                    expectation: token("+"),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: token("-"),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: token("*"),
                },
                Trace {
                    context: vec![frame("Op", 0)],
                    expectation: token("/"),
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
            expectation: token("+"),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: token("-"),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: token("*"),
        },
        Trace {
            context: vec![frame("NullableOp", 0)],
            expectation: token("y"),
        },
        Trace {
            context: vec![frame("NullableOpThenTail", 0)],
            expectation: token("TAIL"),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: token("b"),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: token("c"),
        },
        Trace {
            context: vec![frame("Outermost", 0)],
            expectation: token("d"),
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
