//! `parse_prefix` and `check_prefix`, and `Error` as a `std::error::Error`.

use tygr::*;

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct Digits(StringOf1<Digit>);

#[test]
fn parse_prefix_stops_at_the_match_without_consuming_the_rest() {
    let (value, pos) = Digits::parse_prefix::<Value, Max>("123abc").unwrap();
    assert_eq!(value.print(), "123");
    assert_eq!(pos, 3);
}

#[test]
fn parse_prefix_fails_when_nothing_matches() {
    assert_eq!(
        Digits::parse_prefix::<Value, Max>("abc")
            .unwrap_err()
            .to_string(),
        "parse error at byte 0, expected:\n\t- Digits"
    );
}

#[test]
fn check_prefix_stops_at_the_match_without_consuming_the_rest() {
    assert_eq!(Digits::parse_prefix::<Unit, Max>("123abc").unwrap().1, 3);
}

#[test]
fn check_prefix_fails_when_nothing_matches() {
    assert_eq!(
        Digits::parse_prefix::<Unit, Max>("abc")
            .unwrap_err()
            .to_string(),
        "parse error at byte 0, expected:\n\t- Digits"
    );
}

#[test]
fn error_is_a_std_error() {
    let err = Digits::parse::<Value, Max>("abc").unwrap_err();
    let _: Box<dyn std::error::Error> = Box::new(err);
}
