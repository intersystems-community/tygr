use tygr::*;

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Digits(StringOf1<IsDigit>);

#[test]
fn parse_prefix_stops_at_the_match_without_consuming_the_rest() {
    let (value, pos) = Digits::parse_prefix("123abc").unwrap();
    assert_eq!(value.print(), "123");
    assert_eq!(pos, 3);
}

#[test]
fn parse_prefix_fails_when_nothing_matches() {
    assert!(Digits::parse_prefix("abc").is_err());
}

#[test]
fn scan_prefix_stops_at_the_match_without_consuming_the_rest() {
    assert_eq!(Digits::scan_prefix("123abc").unwrap(), 3);
}

#[test]
fn scan_prefix_fails_when_nothing_matches() {
    assert!(Digits::scan_prefix("abc").is_err());
}
