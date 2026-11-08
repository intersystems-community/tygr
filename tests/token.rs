//! Terminal grammars: literals, `char` and character classes.

use tygr::*;

// "é" is 0xC3 0xA9 in UTF-8; "è" is 0xC3 0xA8.

#[derive(Grammar, Debug)]
struct Accent(StringEqCI!("é"));

#[cfg(feature = "trace_one_node")]
fn token(text: &'static str) -> Expectation {
    Expectation::Token(text.as_bytes())
}

#[test]
fn string_eq_ci_matches_unicode_case_variants() {
    assert_eq!(Accent::parse("é").unwrap().print(), "é");
    assert_eq!(Accent::parse("É").unwrap().print(), "É");
    assert!(Accent::parse("e").is_err());
}

#[derive(Grammar, Debug)]
struct Strasse(StringEqCI!("straße"));

#[test]
fn string_eq_ci_variant_can_change_length() {
    // 'ß' uppercases to "SS".
    assert_eq!(Strasse::parse("STRASSE").unwrap().print(), "STRASSE");
    assert!(Strasse::parse("Straße").is_ok());
}

#[derive(Grammar, Debug)]
struct Long(StringEq!("abcdefghijklmnop"));

#[test]
fn string_eq_longer_than_a_flat_tuple() {
    assert!(Long::parse("abcdefghijklmnop").is_ok());
    assert!(Long::parse("abcdefghijklmnoq").is_err());
}

#[test]
fn char_decodes_one_character_of_any_width() {
    for s in ["a", "é", "平", "😀"] {
        let (ch, end) = char::parse_prefix(s).unwrap();
        assert_eq!((ch.to_string().as_str(), end), (s, s.len()));
    }
    assert!(char::parse("").is_err());
}

#[cfg(feature = "trace_one_node")]
#[test]
fn literal_errors_report_the_whole_literal_at_its_start() {
    #[derive(Grammar, Debug)]
    struct Arrow(StringEq!("->"));
    let err = Arrow::parse("-x").unwrap_err();
    assert_eq!(err.pos(), 0);
    assert_eq!(err.traces()[0].expectation, token("->"));

    // Even a mismatch inside a multi-byte char.
    #[derive(Grammar, Debug)]
    struct E(StringEq!("aé"));
    let err = E::parse("aè").unwrap_err();
    assert_eq!(err.pos(), 0);
    assert_eq!(err.traces()[0].expectation, token("aé"));

    // The case variants of one char give one expectation, not one each.
    #[derive(Grammar, Debug)]
    struct Kw(StringEqCI!("if"));
    let err = Kw::parse("x").unwrap_err();
    assert_eq!(err.traces().len(), 1);
    assert_eq!(err.traces()[0].expectation, token("if"));
}

#[test]
fn ci_marker_only_where_case_matters() {
    #[derive(Grammar)]
    #[tygr(name = "Num")]
    struct Num(StringEqCI!("123"));
    #[derive(Grammar)]
    #[tygr(name = "Kw")]
    struct Kw(StringEqCI!("if"));
    assert_eq!(Num::bnf_rule(), "Num = \"123\" .");
    assert_eq!(Kw::bnf_rule(), "Kw = \"if\"i .");
}

#[test]
fn char_class_computes_first_bytes() {
    char_class!(Lower, |ch| ch.is_ascii_lowercase());
    let starts = |b| tygr::first::contains::<<Lower as Parse>::First>(Some(b));
    assert!(starts(b'a') && starts(b'z'));
    assert!(!starts(b'A') && !starts(b'0'));
    // UTF-8 lead bytes are kept (the predicate isn't evaluated beyond
    // ASCII); continuation bytes can't start a char.
    assert!(starts(0xC3) && !starts(0x80));
    assert!(!tygr::first::contains::<<Lower as Parse>::First>(None));

    #[derive(Grammar, Debug)]
    #[tygr(name = "Word")]
    struct Word(StringOf1<Lower>);
    assert_eq!(Word::parse("abc").unwrap().print(), "abc");
    assert!(Word::parse("aBc").is_err());
}

#[test]
fn char_first_is_ascii_and_lead_bytes() {
    let starts = |b| tygr::first::contains::<<char as Parse>::First>(Some(b));
    assert!(starts(b'a') && starts(0x00) && starts(0x7F));
    assert!(starts(0xC2) && starts(0xE4) && starts(0xF4));
    assert!(!starts(0x80) && !starts(0xBF) && !starts(0xC1));
    assert!(!starts(0xF5) && !starts(0xFF));
}
