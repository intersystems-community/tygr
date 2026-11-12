//! Items with separators between them, `T { S T }`, as `(T, Vec<(S, T)>)`.

use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct List(StringOf1<Digit>, Vec<(StringEq!(","), StringOf1<Digit>)>);

#[test]
fn items_and_separators_line_up() {
    let (list, end) = List::parse_prefix("1,22,3").unwrap();
    assert_eq!(end, 6);
    let rest: Vec<&str> = list.1.iter().map(|(_, item)| item.as_str()).collect();
    assert_eq!((list.0.as_str(), rest), ("1", vec!["22", "3"]));
    assert_eq!(list.print(), "1,22,3");
}

#[test]
fn a_trailing_separator_is_left_unconsumed() {
    let (list, end) = List::parse_prefix("1,2,").unwrap();
    assert_eq!((list.1.len(), end), (1, 3));
    assert!(List::parse("1,2,").is_err());
}

#[test]
fn at_least_one_item_is_required() {
    assert!(List::parse("").is_err());
}

#[test]
fn bnf_is_item_then_separated_items() {
    assert_eq!(
        rule::<List>(),
        r#"List = 'Digit' { 'Digit' } { "," 'Digit' { 'Digit' } } ."#
    );
}
