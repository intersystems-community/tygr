//! `Either`, `Prefix`, `Suffix` and `Hidden`, parsed, printed and rendered.

use either::Either;
use tygr::bnf::Expr;
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
    let s = Shorthands::parse("a-12!;").unwrap();
    assert!(s.0.is_left());
    assert_eq!(s.1.0.as_str(), "12");
    assert_eq!(s.print(), "a-12!;");
    assert_eq!(
        Shorthands::parse("7-12!;")
            .unwrap()
            .0
            .right()
            .unwrap()
            .0
            .as_str(),
        "7"
    );
    assert!(Shorthands::parse("a-12!").is_err());
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
    assert!(Never::check("").is_err());
    assert!(std::convert::Infallible::check("").is_err());
    assert_eq!(Quoted::<Never>::check("\"\""), Ok(()));
    let err = Quoted::<Never>::check("\"a\"").unwrap_err();
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
    assert_eq!(DigitOrNothing::check("7"), Ok(()));
    assert_eq!(rule::<DigitOrNothing>(), "DigitOrNothing = 'digit' .");
    assert!(DigitThenNothing::check("7").is_err());
    assert_eq!(rule::<DigitThenNothing>(), "DigitThenNothing = 'digit' ∅ .");
}

#[derive(Grammar, Debug)]
struct OptOpt(Option<Option<Digit>>, Digit);

#[derive(Grammar, Debug)]
enum DigitOrNot {
    Digit(Digit),
    Not,
}

#[test]
fn an_optional_is_an_alternation_with_an_empty_alternative() {
    assert_eq!(rule::<OptOpt>(), "OptOpt = [ 'digit' ] 'digit' .");
    assert_eq!(rule::<DigitOrNot>(), "DigitOrNot = [ 'digit' ] .");
    assert_eq!(
        Expr::alternation(vec![Expr::empty(), Expr::empty()]),
        Expr::Alternation(vec![Expr::empty(), Expr::empty()])
    );
    assert_eq!(
        Expr::sequence(vec![Expr::alternation(vec![Expr::empty(), Expr::empty()])]).to_string(),
        ""
    );
}
