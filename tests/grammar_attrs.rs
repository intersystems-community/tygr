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
fn hidden_field_vanishes_from_the_outer_rule_but_still_parses_and_prints() {
    assert_eq!(UsesHidden::bnf_rule(), "UsesHidden = Digits .");
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
    fn print_to(&self, buf: &mut String) {
        buf.push_str(&self.0.to_string());
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
    fn print_to(&self, buf: &mut String) {
        buf.push_str(&(self.0 / 2).to_string());
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
