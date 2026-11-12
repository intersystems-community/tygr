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
