//! Terminal grammars: literals, `char` and character classes.

use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

#[derive(Grammar, Debug)]
struct Accent(StringEqCI!("é"));

#[test]
fn string_eq_ci_matches_unicode_case_variants() {
    assert_eq!(Accent::parse::<Value, Max>("é").unwrap().print(), "é");
    assert_eq!(Accent::parse::<Value, Max>("É").unwrap().print(), "É");
    assert!(Accent::parse::<Value, Max>("e").is_err());
}

#[derive(Grammar, Debug)]
struct Strasse(StringEqCI!("straße"));

#[test]
fn string_eq_ci_variant_can_change_length() {
    // 'ß' uppercases to "SS".
    assert_eq!(
        Strasse::parse::<Value, Max>("STRASSE").unwrap().print(),
        "STRASSE"
    );
    assert!(Strasse::parse::<Value, Max>("strasse").is_ok());
    assert!(Strasse::parse::<Value, Max>("Straße").is_ok());
}

#[derive(Grammar, Debug)]
struct Long(
    StringEq!(
        "abcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmnopabcdefghijklmnop"
    ),
);

#[test]
fn string_eq_longer_than_a_flat_tuple() {
    let text = "abcdefghijklmnop".repeat(8);
    assert!(Long::parse::<Value, Max>(&text).is_ok());
    assert!(Long::parse::<Value, Max>(&text[..127]).is_err());
}

#[test]
fn char_decodes_one_character_of_any_width() {
    for s in ["a", "é", "平", "😀"] {
        let (ch, end) = char::parse_prefix::<Value, Max>(s).unwrap();
        assert_eq!((ch.to_string().as_str(), end), (s, s.len()));
    }
    assert!(char::parse::<Value, Max>("").is_err());
}

#[test]
fn literal_errors_report_the_whole_literal_where_it_diverged() {
    fn token(text: &'static str, range: std::ops::Range<usize>) -> Expectation {
        Expectation::Token { text, range }
    }

    #[derive(Grammar, Debug)]
    struct Arrow(StringEq!("->"));
    let err = Arrow::parse::<Value, Max>("-x").unwrap_err();
    assert_eq!(err.pos(), 1);
    assert_eq!(err.traces()[0].expectation, token("->", 0..1));

    // A mismatch inside a multi-byte char backs out to the char's start
    // ("é" is 0xC3 0xA9, "è" is 0xC3 0xA8).
    #[derive(Grammar, Debug)]
    struct E(StringEq!("aé"));
    let err = E::parse::<Value, Max>("aè").unwrap_err();
    assert_eq!(err.pos(), 1);
    assert_eq!(err.traces()[0].expectation, token("aé", 0..1));

    // The range is the input matched, even where a case variant is longer.
    let err = Strasse::parse::<Value, Max>("STRASSX").unwrap_err();
    assert_eq!(err.pos(), 6);
    assert_eq!(err.traces()[0].expectation, token("straße", 0..6));
}

#[test]
fn ci_marker_only_where_case_matters() {
    #[derive(Grammar)]
    struct Num(StringEqCI!("123"));
    #[derive(Grammar)]
    struct Kw(StringEqCI!("if"));
    assert_eq!(rule::<Num>(), "Num = \"123\" .");
    assert_eq!(rule::<Kw>(), "Kw = \"if\"i .");
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
    struct Word(StringOf1<Lower>);
    assert_eq!(Word::parse::<Value, Max>("abc").unwrap().print(), "abc");
    assert!(Word::parse::<Value, Max>("aBc").is_err());
}

#[test]
fn char_first_is_ascii_and_lead_bytes() {
    let starts = |b| tygr::first::contains::<<char as Parse>::First>(Some(b));
    assert!(starts(b'a') && starts(0x00) && starts(0x7F));
    assert!(starts(0xC2) && starts(0xE4) && starts(0xF4));
    assert!(!starts(0x80) && !starts(0xBF) && !starts(0xC1));
    assert!(!starts(0xF5) && !starts(0xFF));
}

#[test]
fn bnf_escapes_only_the_quote_in_use() {
    char_class!(Quote, "\" or '", |ch| matches!(ch, '"' | '\''));
    #[derive(Grammar)]
    struct Quoted(StringEq!("\"'"), Quote);
    assert_eq!(rule::<Quoted>(), r#"Quoted = "\"'" '" or \'' ."#);
}
