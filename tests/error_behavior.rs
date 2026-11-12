//! What a failed parse records, and how its error reads.

use std::ops::Range;
use tygr::*;

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

fn token(text: &'static str, range: Range<usize>) -> Expectation {
    Expectation::Token { text, range }
}

fn frame(node: &'static str, pos: usize) -> Frame {
    Frame { node, pos }
}

/// A trace from top-level rule `node`, entered at 0, expecting `expectation`.
fn trace(node: &'static str, expectation: Expectation) -> Trace {
    Trace {
        context: vec![frame(node, 0)],
        expectation,
    }
}

/// Check that `err` traced `traces` at `pos`.
fn assert_error(err: Error, traces: Vec<Trace>, pos: usize) {
    assert_eq!((err.pos(), err.traces()), (pos, &traces[..]));
}

#[derive(Grammar, Debug)]
struct Lit(StringEq!("foo"));

#[test]
fn string_eq_records_literal() {
    let err = Lit::parse("bar").unwrap_err();
    assert_error(err, vec![trace("Lit", token("foo", 0..0))], 0);
}

#[derive(Grammar, Debug)]
struct LitCI(StringEqCI!("foo"));

#[test]
fn string_eq_ci_records_literal() {
    let err = LitCI::parse("bar").unwrap_err();
    assert_error(err, vec![trace("LitCI", token("foo", 0..0))], 0);
}

#[derive(Grammar, Debug)]
struct Arrow(StringEq!("->"));

#[derive(Grammar, Debug)]
struct Digits(StringOf1<Digit>);

#[test]
fn char_class_records_class_name() {
    let err = Digits::parse("abc").unwrap_err();
    assert_error(
        err,
        vec![trace("Digits", Expectation::CharClass("digit"))],
        0,
    );
}

#[derive(Parse, Debug)]
struct SmallNumber(#[tygr(from_str = StringOf1<Digit>)] u8);

#[test]
fn conversion_records_requirement() {
    let err = SmallNumber::parse("999").unwrap_err();
    assert_error(
        err,
        vec![
            trace("SmallNumber", Expectation::CharClass("digit")),
            trace(
                "SmallNumber",
                Expectation::Rejected {
                    node: None,
                    range: 0..3,
                    requirement: "be convertible to u8".into(),
                },
            ),
        ],
        3,
    );
}

#[derive(Grammar, Debug)]
#[tygr(validated)]
struct NonZero(StringOf1<Digit>);

impl Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[test]
fn validated_records_requirement() {
    let err = NonZero::parse("007").unwrap_err();
    assert_error(
        err,
        vec![
            trace("NonZero", Expectation::CharClass("digit")),
            trace(
                "NonZero",
                Expectation::Rejected {
                    node: Some("NonZero"),
                    range: 0..3,
                    requirement: "be non-zero".into(),
                },
            ),
        ],
        3,
    );
}

#[test]
fn display_names_a_rule_starting_at_the_error_else_its_expectation() {
    let cases = [
        (
            Lit::parse("bar").unwrap_err(),
            "parse error at byte 0, expected:\n\t- Lit",
        ),
        (
            LitCI::parse("bar").unwrap_err(),
            "parse error at byte 0, expected:\n\t- LitCI",
        ),
        (
            Arrow::parse("-x").unwrap_err(),
            "parse error at byte 1, expected:\n\t- \"->\" at 0..1",
        ),
        (
            Digits::parse("abc").unwrap_err(),
            "parse error at byte 0, expected:\n\t- Digits",
        ),
        (
            SmallNumber::parse("999").unwrap_err(),
            "parse error at byte 3, expected:\n\t- 'digit'\n\t- Text at 0..3 must be convertible to u8",
        ),
        (
            NonZero::parse("007").unwrap_err(),
            "parse error at byte 3, expected:\n\t- 'digit'\n\t- NonZero at 0..3 must be non-zero",
        ),
    ];

    for (err, expected) in cases {
        assert_eq!(err.to_string(), expected);
    }
}

#[test]
fn display_escapes_tokens() {
    assert_eq!(token("\\\"\n", 0..0).to_string(), r#""\\\"\n""#);
}

#[derive(Grammar, Debug)]
struct Wrapper(Lit);

#[test]
fn context_records_the_rule_chain() {
    let err = Wrapper::parse("bar").unwrap_err();
    assert_eq!(
        format!("{err:?}"),
        "parse error at byte 0, expected:\n\t- Wrapper@0/Lit@0/\"foo\""
    );
    assert_error(
        err,
        vec![Trace {
            context: vec![frame("Wrapper", 0), frame("Lit", 0)],
            expectation: token("foo", 0..0),
        }],
        0,
    );
}

// 4+ variants trips the derive's FIRST-set dispatch table (see `THRESHOLD`
// in tygr-derive's `dispatch.rs`), which the test below exercises.
#[derive(Grammar, Debug)]
enum Op {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
    Mul(StringEq!("*")),
    Div(StringEq!("/")),
}

#[test]
fn enum_records_every_variant_when_first_matches_none() {
    let err = Op::parse("x").unwrap_err();
    assert_error(
        err,
        vec![
            trace("Op", token("+", 0..0)),
            trace("Op", token("-", 0..0)),
            trace("Op", token("*", 0..0)),
            trace("Op", token("/", 0..0)),
        ],
        0,
    );
}

// A nullable variant (`Option<..>` always matches empty) inside a 4+-variant
// enum, embedded as a non-last struct field, embedded as a non-last variant of
// another 4+-variant enum — exercises fail_at's OR-chain (nullable variant
// doesn't block the struct's next field) and AND-fold (a required sibling
// variant doesn't block the enum's other siblings) together.
#[derive(Grammar, Debug)]
enum NullableOp {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
    Mul(StringEq!("*")),
    Maybe(Option<StringEq!("y")>),
}

#[derive(Grammar, Debug)]
struct NullableOpThenTail(NullableOp, StringEq!("TAIL"));

#[derive(Grammar, Debug)]
enum Outermost {
    A(NullableOpThenTail),
    B(StringEq!("b")),
    C(StringEq!("c")),
    D(StringEq!("d")),
}

fn nullable_variant_traces() -> Vec<Trace> {
    let outermost = vec![frame("Outermost", 0)];
    let tail = [&outermost[..], &[frame("NullableOpThenTail", 0)]].concat();
    let op = [&tail[..], &[frame("NullableOp", 0)]].concat();
    let expected = |context: &[Frame], text| Trace {
        context: context.to_vec(),
        expectation: token(text, 0..0),
    };
    vec![
        expected(&op, "+"),
        expected(&op, "-"),
        expected(&op, "*"),
        expected(&op, "y"),
        expected(&tail, "TAIL"),
        expected(&outermost, "b"),
        expected(&outermost, "c"),
        expected(&outermost, "d"),
    ]
}

#[test]
fn nullable_variant_does_not_block_sibling_field_or_sibling_variant() {
    let err = Outermost::parse("x").unwrap_err();
    assert_error(err, nullable_variant_traces(), 0);
}

#[test]
fn leftover_input_expects_the_end() {
    let end = || {
        vec![Trace {
            context: vec![],
            expectation: Expectation::End,
        }]
    };
    assert_error(Lit::parse("foox").unwrap_err(), end(), 3);
    assert_error(Lit::check("foox").unwrap_err(), end(), 3);
    assert_eq!(
        Lit::parse("foox").unwrap_err().to_string(),
        "parse error at byte 3, expected:\n\t- end of input"
    );
}

/// The expectations `err` traced, printed and sorted.
fn expectations(err: &Error) -> Vec<String> {
    let mut expectations: Vec<String> = err
        .traces()
        .iter()
        .map(|trace| trace.expectation.to_string())
        .collect();
    expectations.sort();
    expectations
}

#[derive(Grammar, Debug)]
enum Four {
    A(StringEq!("a")),
    B(StringEq!("b")),
    C(StringEq!("cx")),
    D(StringEq!("é")),
}

#[test]
fn enum_dispatch_records_the_variants_it_skips() {
    // The table jumps to `D` on the first byte "é" shares with "è"; `D`
    // diverges inside that char, so at byte 0, where the others could start.
    let err = Four::parse("è").unwrap_err();
    assert_eq!(err.pos(), 0);
    assert_eq!(
        expectations(&err),
        [r#""a""#, r#""b""#, r#""cx""#, r#""é""#]
    );
    // `C` gets further than any variant the table skipped.
    let err = Four::parse("cy").unwrap_err();
    assert_eq!(err.pos(), 1);
    assert_eq!(expectations(&err), [r#""cx" at 0..1"#]);
}

#[derive(Grammar, Debug)]
enum EitherOrThree {
    AorB(either::Either<StringEq!("a"), StringEq!("b")>),
    C(StringEq!("c")),
    D(StringEq!("d")),
    E(StringEq!("e")),
}

#[test]
fn enum_dispatch_records_both_sides_of_a_skipped_either_variant() {
    let err = EitherOrThree::parse("z").unwrap_err();
    assert_eq!(
        expectations(&err),
        [r#""a""#, r#""b""#, r#""c""#, r#""d""#, r#""e""#]
    );
}

#[derive(Grammar, Debug)]
enum FourOrNone {
    A(StringEq!("a")),
    B(StringEq!("b")),
    C(StringEq!("c")),
    D(Option<StringEq!("d")>),
}

#[derive(Grammar, Debug)]
struct FourOrNoneThenBang(FourOrNone, StringEq!("!"));

#[test]
fn enum_dispatch_records_the_variants_it_skips_when_matching_empty() {
    let err = FourOrNoneThenBang::parse("x").unwrap_err();
    assert_eq!(
        expectations(&err),
        [r#""!""#, r#""a""#, r#""b""#, r#""c""#, r#""d""#]
    );
}

#[test]
fn enum_dispatch_at_end_of_input() {
    assert!(matches!(
        FourOrNone::parse("").unwrap(),
        FourOrNone::D(None)
    ));
    let err = Four::parse("").unwrap_err();
    assert_eq!(
        expectations(&err),
        [r#""a""#, r#""b""#, r#""cx""#, r#""é""#]
    );
}

#[derive(Grammar, Debug)]
struct Div(StringEq!("/"), NotFollowedBy<StringEq!("/")>);

#[derive(Grammar, Debug)]
struct NonOctal(NotFollowedBy<StringEq!("0")>, Digits);

#[derive(Parse, Debug)]
#[tygr(inline)]
struct InlineSmallNumber(#[tygr(from_str = StringOf1<Digit>)] u8);

#[derive(Parse, Debug)]
struct NotSmallNumber(NotFollowedBy<InlineSmallNumber>, StringEq!("x"));

#[test]
fn negative_lookahead_records_what_it_ruled_out() {
    let err = Div::parse("//").unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at byte 1, expected:\n\t- anything but \"/\" at 1..2"
    );
    assert_error(
        err,
        vec![trace(
            "Div",
            Expectation::NotFollowedBy {
                excluded: bnf::Lazy(<StringEq!("/")>::to_bnf),
                range: 1..2,
            },
        )],
        1,
    );
    // Leading, it isn't hidden behind the rule's name.
    assert_eq!(
        NonOctal::parse("07").unwrap_err().to_string(),
        "parse error at byte 0, expected:\n\t- anything but \"0\" at 0..1"
    );
    // An inline grammar's side-conditions are footnoted on the line.
    assert_eq!(
        NotSmallNumber::parse("12x").unwrap_err().to_string(),
        "parse error at byte 0, expected:\n\t- anything but ( 'digit' { 'digit' } ) ^1 at 0..2 (^1: be convertible to u8)"
    );
}

#[derive(Grammar, Debug)]
struct Key(StringEq!("k"), FollowedBy<StringEq!(":")>);

#[derive(Grammar, Debug)]
struct DigitsThenX(FollowedBy<Digits>, StringEq!("x"));

#[test]
fn positive_lookahead_records_what_it_expected() {
    assert_error(
        Key::parse("k").unwrap_err(),
        vec![trace("Key", token(":", 1..1))],
        1,
    );
    // Matching, it records nothing: not the 'digit' `Digits` stopped at.
    assert_error(
        DigitsThenX::parse("12").unwrap_err(),
        vec![trace("DigitsThenX", token("x", 0..0))],
        0,
    );
}

// `S = "a" S "b" | "a" S "c" | ()` backtracks exponentially, expecting the
// same few things at the same position each time.
#[derive(Grammar, Debug)]
enum Exponential {
    B(StringEq!("a"), Box<Exponential>, StringEq!("b")),
    C(StringEq!("a"), Box<Exponential>, StringEq!("c")),
    Empty,
}

#[test]
fn repeated_traces_are_reported_once() {
    let input = format!("{}x", "a".repeat(18));
    let err = Exponential::parse(&input).unwrap_err();
    assert_eq!(err.pos(), 18);
    assert_eq!(expectations(&err), [r#""a""#, r#""b""#, r#""c""#]);
}
