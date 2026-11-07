use tygr::*;

char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
#[grammar(name = "List")]
struct List(VecSep<StringOf1<Digit>, StringEq!(",")>);

#[test]
fn items_and_separators_line_up() {
    let (list, end) = List::parse_prefix("1,22,3").unwrap();
    assert_eq!(end, 6);
    let items: Vec<&str> = list.0.items.iter().map(|item| item.as_str()).collect();
    assert_eq!(items, ["1", "22", "3"]);
    assert_eq!(list.0.seps.len(), 2);
    assert_eq!(list.print(), "1,22,3");
}

#[test]
fn a_trailing_separator_is_left_unconsumed() {
    let (list, end) = List::parse_prefix("1,2,").unwrap();
    assert_eq!((list.0.items.len(), end), (2, 3));
    assert!(List::parse("1,2,").is_err());
    assert!(List::parse("").is_err());
}

#[test]
fn bnf_is_item_then_separated_items() {
    assert_eq!(
        List::bnf_rule(),
        format!(
            "List = '{d}' {{ '{d}' }} {{ \",\" '{d}' {{ '{d}' }} }} .",
            d = if cfg!(feature = "lower_bnf_name") && !cfg!(feature = "upper_bnf_name") {
                "digit"
            } else if cfg!(feature = "upper_bnf_name") && !cfg!(feature = "lower_bnf_name") {
                "DIGIT"
            } else {
                "Digit"
            }
        )
    );
}
