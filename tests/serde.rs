#![cfg(feature = "serde")]

use tygr::*;

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Digits(StringOf1<IsDigit>);

#[derive(Grammar, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
enum Op {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
}

#[derive(Grammar, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Expr {
    left: Digits,
    op: Op,
    right: Digits,
}

#[test]
fn string_eq_serializes_as_unit_and_round_trips() {
    let value = Op::Add(Default::default());
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"Add":null}"#);
    assert_eq!(serde_json::from_str::<Op>(&json).unwrap(), value);
}

#[test]
fn string_of1_serializes_as_the_matched_text() {
    let value = Digits::parse("123").unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#""123""#);
    assert_eq!(serde_json::from_str::<Digits>(&json).unwrap(), value);
}

#[test]
fn expr_serializes_as_a_real_structure_and_round_trips() {
    let value = Expr::parse("12+34").unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"left":"12","op":{"Add":null},"right":"34"}"#);
    assert_eq!(serde_json::from_str::<Expr>(&json).unwrap(), value);
}

#[test]
fn not_followed_by_serializes_as_unit_and_round_trips() {
    let value = NotFollowedBy::<StringEq!("x")>::default();
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, "null");
    let _: NotFollowedBy<StringEq!("x")> = serde_json::from_str(&json).unwrap();
}

#[derive(Grammar, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct CaseInsensitive(StringEqCI!("class"));

#[test]
fn string_eq_ci_serializes_as_the_matched_casing() {
    let value = CaseInsensitive::parse("CLASS").unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#""CLASS""#);
    assert_eq!(
        serde_json::from_str::<CaseInsensitive>(&json).unwrap(),
        value
    );
}

#[derive(Grammar, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Wrapped(Wrap<StringEq!("("), Digits, StringEq!(")")>);

#[test]
fn wrap_serializes_before_wrapped_after_as_named_fields() {
    let value = Wrapped::parse("(42)").unwrap();
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(json, r#"{"before":null,"wrapped":"42","after":null}"#);
    assert_eq!(serde_json::from_str::<Wrapped>(&json).unwrap(), value);
}
