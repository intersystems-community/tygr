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
    type Result = bool;
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
