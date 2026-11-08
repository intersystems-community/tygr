// Expected BNF spells default rule names; `bnf_names.rs` covers renaming.
#![cfg(not(any(feature = "lower_bnf_name", feature = "upper_bnf_name")))]

use std::str::FromStr;
use tygr::*;

char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq)]
struct Digits(StringOf1<IsDigit>);

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesPlain(Digits, Digits);

#[test]
fn no_attribute_uses_type_name_and_references_by_rule() {
    assert_eq!(Digits::NAME, "Digits");
    assert_eq!(UsesPlain::bnf_rule(), "UsesPlain = Digits Digits .");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(name = "custom_name")]
struct Named(StringEq!("x"));

#[test]
fn name_overrides_the_bnf_rule_name() {
    assert_eq!(Named::NAME, "custom_name");
    assert_eq!(Named::bnf_rule(), "custom_name = \"x\" .");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(hidden)]
struct Semi(StringEq!(";"));

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesHidden(Digits, Semi);

#[test]
#[cfg_attr(not(feature = "print"), allow(unused_variables))]
fn hidden_field_vanishes_from_the_outer_rule_but_still_parses_and_prints() {
    assert_eq!(UsesHidden::bnf_rule(), "UsesHidden = Digits .");
    let v = UsesHidden::parse("1;").unwrap();
    #[cfg(feature = "print")]
    assert_eq!(v.print(), "1;");
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesOptionalHidden(Option<Semi>, Digits);

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesRepeatedHidden(Vec<Semi>, Digits);

#[test]
fn optional_or_repeated_hidden_field_vanishes_instead_of_showing_empty_brackets() {
    assert_eq!(
        UsesOptionalHidden::bnf_rule(),
        "UsesOptionalHidden = Digits ."
    );
    assert_eq!(
        UsesRepeatedHidden::bnf_rule(),
        "UsesRepeatedHidden = Digits ."
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(inline)]
struct Inlined(StringEq!("a"), StringEq!("b"));

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesInlined(Inlined, Digits);

#[test]
fn inline_splices_fields_instead_of_referencing_by_rule() {
    assert_eq!(Inlined::bnf_rule(), "Inlined = \"a\" \"b\" .");
    assert_eq!(
        UsesInlined::bnf_rule(),
        "UsesInlined = \"a\" \"b\" Digits ."
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(name = "spliced")]
#[grammar(inline)]
struct TwoAttributes(StringEq!("a"));

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesTwoAttributes(TwoAttributes, Digits);

#[test]
fn separate_attributes_combine() {
    assert_eq!(TwoAttributes::NAME, "spliced");
    assert_eq!(
        UsesTwoAttributes::bnf_rule(),
        "UsesTwoAttributes = \"a\" Digits ."
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(validated)]
struct NonZero(StringOf1<IsDigit>);

impl Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[test]
fn validated_accepts_values_that_pass_validation() {
    #[cfg(feature = "print")]
    assert_eq!(NonZero::parse("123").unwrap().print(), "123");
}

#[test]
fn validated_rejects_values_that_fail_validation() {
    assert!(NonZero::parse("007").is_err());
}

#[test]
fn validated_bnf_shows_the_side_condition() {
    assert_eq!(
        NonZero::bnf_rule(),
        "NonZero = ( 'digit' { 'digit' } ) ^1 .\n\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(validated)]
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
        ValidatedEnum::bnf_rule(),
        "ValidatedEnum = ( \"a\" | \"b\" ) ^1 .\n\n^1: be a recognized token"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(inline, validated)]
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
        UsesInlineValidated::bnf_rule(),
        "UsesInlineValidated = ( ( 'digit' { 'digit' } ) ^1 ) Digits .\n\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug, PartialEq, Eq)]
struct UsesInlineValidatedTwice(InlineNonZero, InlineNonZero);

#[test]
fn repeated_inline_validated_side_condition_gets_one_deduplicated_footnote() {
    assert_eq!(
        UsesInlineValidatedTwice::bnf_rule(),
        "UsesInlineValidatedTwice = ( ( 'digit' { 'digit' } ) ^1 ) ( ( 'digit' { 'digit' } ) ^1 ) .\n\n^1: be non-zero"
    );
}

#[derive(Debug, GrammarFromStr)]
#[grammar(validated)]
struct EvenNumber(u32);

impl GrammarFrom for EvenNumber {
    type Source = StringOf1<IsDigit>;
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.to_string().as_bytes());
    }
}

impl FromStr for EvenNumber {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(EvenNumber)
    }
}

impl Validate for EvenNumber {
    const REQUIREMENT: &'static str = "be even";
    fn validate(&self) -> bool {
        self.0.is_multiple_of(2)
    }
}

#[test]
fn validated_grammar_from_str_bnf_shows_both_side_conditions() {
    assert_eq!(
        EvenNumber::bnf_rule(),
        "EvenNumber = ( ( 'digit' { 'digit' } ) ^1 ) ^2 .\n\n^1: be convertible\n^2: be even"
    );
}

#[test]
fn validated_grammar_from_str_rejects_values_that_fail_validation() {
    assert!(EvenNumber::parse("4").is_ok());
    assert!(EvenNumber::parse("3").is_err());
}

#[derive(Debug, GrammarFromOther)]
#[grammar(validated)]
struct PositiveDoubled(u32);

impl GrammarFrom for PositiveDoubled {
    type Source = StringOf1<IsDigit>;
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice((self.0 / 2).to_string().as_bytes());
    }
}

impl From<StringOf1<IsDigit>> for PositiveDoubled {
    fn from(s: StringOf1<IsDigit>) -> Self {
        PositiveDoubled(s.to_string().parse::<u32>().unwrap_or(0) * 2)
    }
}

impl Validate for PositiveDoubled {
    const REQUIREMENT: &'static str = "be non-zero after doubling";
    fn validate(&self) -> bool {
        self.0 != 0
    }
}

#[test]
fn validated_grammar_from_other_bnf_shows_only_the_validated_side_condition() {
    assert_eq!(
        PositiveDoubled::bnf_rule(),
        "PositiveDoubled = ( 'digit' { 'digit' } ) ^1 .\n\n^1: be non-zero after doubling"
    );
}

#[test]
fn validated_grammar_from_other_rejects_via_scan_too() {
    assert!(PositiveDoubled::parse("5").is_ok());
    assert!(PositiveDoubled::parse("0").is_err());
    assert!(PositiveDoubled::scan("0").is_err());
}

static CONVERSIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[derive(Debug, GrammarFromStrInfallible)]
struct Counted(u32);

impl GrammarFrom for Counted {
    type Source = StringOf1<IsDigit>;
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.to_string().as_bytes());
    }
}

impl FromStr for Counted {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        CONVERSIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(Counted(s.len() as u32))
    }
}

#[test]
fn grammar_from_str_infallible_converts_the_text_but_not_when_scanning() {
    use std::sync::atomic::Ordering::Relaxed;
    assert_eq!(Counted::bnf_rule(), "Counted = 'digit' { 'digit' } .");
    let before = CONVERSIONS.load(Relaxed);
    assert!(Counted::scan("123").is_ok());
    assert_eq!(CONVERSIONS.load(Relaxed), before);
    assert_eq!(Counted::parse("123").unwrap().0, 3);
    assert_eq!(CONVERSIONS.load(Relaxed), before + 1);
    assert!(Counted::parse("x").is_err());
}

// Field conversions: `#[grammar(<kind> = S)]`.

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
    #[grammar(from = StringEq!("null"))]
    Null,
    Fields(
        #[grammar(from_str = StringOf1<IsDigit>)] u32,
        StringEq!(","),
        #[grammar(from_str_infallible = StringOf1<IsDigit>)] String,
        StringEq!(";"),
        #[grammar(from = Digit)] u8,
        #[grammar(try_from = Digit)] bool,
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
    #[cfg(feature = "print")]
    assert_eq!(Converted::parse("42,07;51").unwrap().print(), "42,07;51");
    assert_eq!(
        Converted::bnf_rule(),
        "Converted = \"null\" | ( ( 'digit' { 'digit' } ) ^1 ) \",\" 'digit' { 'digit' } \";\" Digit ( Digit ^1 ) .\n\n^1: be convertible"
    );
    for bad in ["42,07;52", "99999999999,07;51", "nul"] {
        assert!(Converted::parse(bad).is_err() && Converted::scan(bad).is_err());
    }
}
