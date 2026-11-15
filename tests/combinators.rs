//! `Either`, `Prefix`, `Suffix` and `Hidden`, parsed, printed and rendered.

use either::Either;
use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct Digits(StringOf1<Digit>);

#[derive(Grammar, Debug)]
struct Shorthands(
    Either<StringEq!("a"), Digits>,
    Prefix<StringEq!("-"), Digits>,
    Suffix<StringEq!("!"), Hidden<StringEq!(";")>>,
);

#[test]
fn either_prefix_suffix_and_hidden_parse_print_and_render() {
    let s = Shorthands::parse::<success::Value, level::Max>("a-12!;").unwrap();
    assert!(s.0.is_left());
    assert_eq!(s.1.0.as_str(), "12");
    assert_eq!(s.print(), "a-12!;");
    assert_eq!(
        Shorthands::parse::<success::Value, level::Max>("7-12!;")
            .unwrap()
            .0
            .right()
            .unwrap()
            .0
            .as_str(),
        "7"
    );
    assert!(Shorthands::parse::<success::Value, level::Max>("a-12!").is_err());
    assert_eq!(
        rule::<Shorthands>(),
        r#"Shorthands = ( "a" | Digits ) "-" Digits "!" ."#
    );
}

type Quote = StringEq!("\"");

#[derive(Grammar, Debug)]
struct Quoted<T>(Quote, Vec<T>, Quote);

#[derive(Grammar, Debug)]
struct DigitOrNothing(Either<Never, Digit>);

#[derive(Grammar, Debug)]
struct DigitThenNothing(Digit, Never);

#[test]
fn never_never_matches() {
    assert!(Never::parse::<success::Unit, level::Max>("").is_err());
    assert!(std::convert::Infallible::parse::<success::Unit, level::Max>("").is_err());
    assert_eq!(
        Quoted::<Never>::parse::<success::Unit, level::Max>("\"\""),
        Ok(())
    );
    let err = Quoted::<Never>::parse::<success::Unit, level::Max>("\"a\"").unwrap_err();
    assert_eq!((err.pos(), err.traces().len()), (1, 1));
    assert_eq!(
        rule::<Quoted<Digit>>(),
        r#"Quoted = "\"" { 'digit' } "\"" ."#
    );
    assert_eq!(rule::<Quoted<Never>>(), r#"Quoted = "\"" { ∅ } "\"" ."#);
    assert_eq!(
        rule::<Quoted<std::convert::Infallible>>(),
        rule::<Quoted<Never>>()
    );
    assert_eq!(
        DigitOrNothing::parse::<success::Unit, level::Max>("7"),
        Ok(())
    );
    assert_eq!(rule::<DigitOrNothing>(), "DigitOrNothing = 'digit' .");
    assert!(DigitThenNothing::parse::<success::Unit, level::Max>("7").is_err());
    assert_eq!(rule::<DigitThenNothing>(), "DigitThenNothing = 'digit' ∅ .");
}
