use std::fmt;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

/// The names of `G`'s rules, in order.
fn names<G: Parse>() -> Vec<String> {
    G::bnf_rules().into_iter().map(|rule| rule.name).collect()
}

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Digits(StringOf1<IsDigit>);

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesPlain(Digits, Digits);

#[test]
fn no_attribute_uses_type_name_and_references_by_rule() {
    assert_eq!(rule::<UsesPlain>(), "UsesPlain = Digits Digits .");
    assert_eq!(names::<UsesPlain>(), ["UsesPlain", "Digits"]);
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(name = "custom_name")]
struct Named(StringEq!("x"));

#[test]
fn name_overrides_the_bnf_rule_name() {
    assert_eq!(rule::<Named>(), "custom_name = \"x\" .");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(hidden)]
struct Semi(StringEq!(";"));

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesHidden(Digits, Semi);

#[test]
fn hidden_field_vanishes_from_the_outer_rule_but_still_parses_and_prints() {
    assert_eq!(rule::<UsesHidden>(), "UsesHidden = Digits .");
    let v = UsesHidden::parse("1;").unwrap();
    assert_eq!(v.print(), "1;");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesOptionalHidden(Option<Semi>, Digits);

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesRepeatedHidden(Vec<Semi>, Digits);

#[test]
fn optional_or_repeated_hidden_field_vanishes_instead_of_showing_empty_brackets() {
    assert_eq!(
        rule::<UsesOptionalHidden>(),
        "UsesOptionalHidden = Digits ."
    );
    assert_eq!(
        rule::<UsesRepeatedHidden>(),
        "UsesRepeatedHidden = Digits ."
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(inline)]
enum MaybeSemi {
    Semi(Semi),
    Digits(Digits),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesHiddenAlternative(MaybeSemi, Named, NotFollowedBy<Semi>);

#[test]
fn hidden_alternative_or_lookahead_vanishes() {
    assert_eq!(
        rule::<UsesHiddenAlternative>(),
        "UsesHiddenAlternative = Digits custom_name ."
    );
    assert!(UsesHiddenAlternative::parse(";x").is_ok());
}

#[derive(Grammar, Debug, PartialEq, Eq)]
enum MaybeDigits {
    Digits(Digits),
    Nothing,
}

#[test]
fn empty_alternative_makes_the_rest_optional() {
    assert_eq!(rule::<MaybeDigits>(), "MaybeDigits = [ Digits ] .");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(inline)]
struct Inlined(StringEq!("a"), StringEq!("b"));

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesInlined(Inlined, Digits);

#[test]
fn inline_splices_fields_instead_of_referencing_by_rule() {
    assert_eq!(rule::<UsesInlined>(), "UsesInlined = \"a\" \"b\" Digits .");
    assert_eq!(names::<UsesInlined>(), ["UsesInlined", "Digits"]);
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(name = "renamed")]
#[tygr(validated)]
struct TwoAttributes(StringEq!("a"));

impl Validate for TwoAttributes {
    const REQUIREMENT: &'static str = "be valid";
    fn validate(&self) -> bool {
        true
    }
}

#[test]
fn separate_attributes_combine() {
    assert_eq!(
        rule::<TwoAttributes>(),
        "renamed = \"a\" ^1 .\n\n^1: be valid"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(validated)]
struct NonZero(StringOf1<IsDigit>);

impl Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[test]
fn validated_accepts_values_that_pass_validation() {
    assert_eq!(NonZero::parse("123").unwrap().print(), "123");
}

#[test]
fn validated_rejects_values_that_fail_validation() {
    assert!(NonZero::parse("007").is_err());
}

#[test]
fn validated_bnf_shows_the_side_condition() {
    assert_eq!(
        rule::<NonZero>(),
        "NonZero = ( 'digit' { 'digit' } ) ^1 .\n\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(validated)]
enum ValidatedEnum {
    A(StringEq!("a")),
    B(StringEq!("b")),
}

impl Validate for ValidatedEnum {
    const REQUIREMENT: &'static str = "be a recognized token";
    fn validate(&self) -> bool {
        true
    }
}

#[test]
fn validated_enum_bnf_shows_the_side_condition_around_the_whole_alternation() {
    assert_eq!(
        rule::<ValidatedEnum>(),
        "ValidatedEnum = ( \"a\" | \"b\" ) ^1 .\n\n^1: be a recognized token"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[tygr(inline, validated)]
struct InlineNonZero(StringOf1<IsDigit>);

impl Validate for InlineNonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesInlineValidated(InlineNonZero, Digits);

#[test]
fn inline_validated_side_condition_propagates_into_the_splice() {
    assert_eq!(
        rule::<UsesInlineValidated>(),
        "UsesInlineValidated = ( ( 'digit' { 'digit' } ) ^1 ) Digits .\n\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesInlineValidatedTwice(InlineNonZero, InlineNonZero);

#[test]
fn repeated_inline_validated_side_condition_gets_one_deduplicated_footnote() {
    assert_eq!(
        rule::<UsesInlineValidatedTwice>(),
        "UsesInlineValidatedTwice = ( ( 'digit' { 'digit' } ) ^1 ) ( ( 'digit' { 'digit' } ) ^1 ) .\n\n^1: be non-zero"
    );
}

// Field conversions: `#[tygr(<kind> = S)]`.

#[derive(Debug, PartialEq, Grammar)]
struct Digit(IsDigit);

impl From<Digit> for u8 {
    fn from(d: Digit) -> u8 {
        d.0.value() as u8 - b'0'
    }
}

impl From<&u8> for Digit {
    fn from(n: &u8) -> Digit {
        Digit(IsDigit::new((b'0' + n) as char))
    }
}

impl TryFrom<Digit> for bool {
    type Error = ();
    fn try_from(d: Digit) -> Result<bool, ()> {
        [false, true].get(u8::from(d) as usize).copied().ok_or(())
    }
}

impl From<&bool> for Digit {
    fn from(b: &bool) -> Digit {
        Digit::from(&(*b as u8))
    }
}

#[derive(Debug, PartialEq, Grammar)]
enum Converted {
    #[tygr(from = StringEq!("null"))]
    Null,
    Fields(
        #[tygr(from_str = StringOf1<IsDigit>)] u32,
        StringEq!(","),
        #[tygr(from_str_infallible = StringOf1<IsDigit>)] String,
        StringEq!(";"),
        #[tygr(from = Digit)] u8,
        #[tygr(try_from = Digit)] bool,
    ),
}

#[test]
fn field_conversions() {
    let Converted::Fields(n, _, text, _, digit, flag) = Converted::parse("42,07;51").unwrap()
    else {
        panic!()
    };
    assert_eq!((n, text.as_str(), digit, flag), (42, "07", 5, true));
    assert_eq!(Converted::parse("null").unwrap(), Converted::Null);
    assert_eq!(Converted::parse("42,07;51").unwrap().print(), "42,07;51");
    assert_eq!(
        rule::<Converted>(),
        "Converted = \"null\" | ( ( 'digit' { 'digit' } ) ^1 ) \",\" 'digit' { 'digit' } \";\" Digit ( Digit ^1 ) .\n\n^1: be convertible"
    );
    for bad in ["42,07;52", "99999999999,07;51", "nul"] {
        assert!(Converted::parse(bad).is_err() && Converted::check(bad).is_err());
    }
}

#[derive(Debug, Grammar)]
#[tygr(validated)]
struct EvenNumber(#[tygr(from_str = StringOf1<IsDigit>)] u32);

impl Validate for EvenNumber {
    const REQUIREMENT: &'static str = "be even";
    fn validate(&self) -> bool {
        self.0.is_multiple_of(2)
    }
}

#[test]
fn validated_fallible_conversion_bnf_shows_both_side_conditions() {
    assert_eq!(
        rule::<EvenNumber>(),
        "EvenNumber = ( ( 'digit' { 'digit' } ) ^1 ) ^2 .\n\n^1: be convertible\n^2: be even"
    );
    assert!(EvenNumber::parse("4").is_ok());
    assert!(EvenNumber::parse("3").is_err());
}

#[derive(Debug, Grammar)]
#[tygr(validated)]
struct NonZeroDigit(#[tygr(from = Digit)] u8);

impl Validate for NonZeroDigit {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        self.0 != 0
    }
}

#[test]
fn validated_infallible_conversion_bnf_shows_only_the_validation() {
    assert_eq!(
        rule::<NonZeroDigit>(),
        "NonZeroDigit = Digit ^1 .\n\n^1: be non-zero"
    );
}

#[test]
fn validated_conversion_rejects_when_checking_too() {
    assert!(NonZeroDigit::parse("5").is_ok());
    assert!(NonZeroDigit::parse("0").is_err());
    assert!(NonZeroDigit::check("0").is_err());
}

static CONVERSIONS: AtomicUsize = AtomicUsize::new(0);

/// Counts its conversions.
#[derive(Debug, PartialEq)]
struct Counted(String);

impl FromStr for Counted {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        CONVERSIONS.fetch_add(1, Relaxed);
        Ok(Counted(s.to_string()))
    }
}

impl fmt::Display for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Grammar)]
struct UsesCounted(#[tygr(from_str_infallible = StringOf1<IsDigit>)] Counted);

#[test]
fn infallible_conversion_is_skipped_when_checking() {
    assert_eq!(rule::<UsesCounted>(), "UsesCounted = 'digit' { 'digit' } .");
    let before = CONVERSIONS.load(Relaxed);
    assert!(UsesCounted::check("123").is_ok());
    assert_eq!(CONVERSIONS.load(Relaxed), before);
    assert_eq!(UsesCounted::parse("123").unwrap().0, Counted("123".into()));
    assert_eq!(CONVERSIONS.load(Relaxed), before + 1);
}

/// Parsed only: a `u32` doesn't print.
#[derive(Parse)]
struct ParsedOnly(#[tygr(from_str = StringOf1<IsDigit>)] u32);

/// Printed only, as when built by hand.
#[derive(Print)]
struct PrintedOnly(StringEq!("#"), Digits);

#[test]
fn parse_and_print_derive_separately() {
    assert_eq!(ParsedOnly::parse("42").unwrap().0, 42);
    let printed = PrintedOnly(Default::default(), Digits(Raw::new("42")));
    assert_eq!(printed.print(), "#42");
}
