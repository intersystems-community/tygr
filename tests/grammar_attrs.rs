//! `#[tygr(...)]` options on types and fields, and the EBNF they give.

use std::marker::PhantomData;
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

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct Digits(StringOf1<Digit>);

#[derive(Grammar, Debug)]
struct UsesPlain(Digits, Digits);

#[test]
fn no_attribute_uses_type_name_and_references_by_rule() {
    assert_eq!(rule::<UsesPlain>(), "UsesPlain = Digits Digits .");
    assert_eq!(names::<UsesPlain>(), ["UsesPlain", "Digits"]);
}

#[derive(Grammar, Debug)]
#[tygr(name = "custom_name")]
struct Named(StringEq!("x"));

#[test]
fn name_overrides_the_bnf_rule_name() {
    assert_eq!(rule::<Named>(), "custom_name = \"x\" .");
}

#[derive(Grammar, Debug)]
#[tygr(hidden)]
struct Semi(StringEq!(";"));

#[derive(Grammar, Debug)]
struct UsesHidden(Digits, Semi);

#[test]
fn hidden_field_vanishes_from_the_outer_rule_but_still_parses_and_prints() {
    assert_eq!(rule::<UsesHidden>(), "UsesHidden = Digits .");
    let v = UsesHidden::parse("1;").unwrap();
    assert_eq!(v.print(), "1;");
}

#[derive(Grammar, Debug)]
struct UsesOptionalHidden(Option<Semi>, Digits);

#[derive(Grammar, Debug)]
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

#[derive(Grammar, Debug)]
#[tygr(inline)]
enum MaybeSemi {
    Semi(Semi),
    Digits(Digits),
}

#[derive(Grammar, Debug)]
struct UsesHiddenAlternative(MaybeSemi, Named, NotFollowedBy<Semi>);

#[test]
fn hidden_alternative_or_lookahead_vanishes() {
    assert_eq!(
        rule::<UsesHiddenAlternative>(),
        "UsesHiddenAlternative = Digits custom_name ."
    );
    assert!(UsesHiddenAlternative::parse(";x").is_ok());
    #[cfg(feature = "trace_one_node")]
    assert_eq!(
        UsesHiddenAlternative::parse("1x;").unwrap_err().to_string(),
        "parse error at byte 2, expected:\n\t- anything but the text at 2..3"
    );
}

#[derive(Parse)]
struct UsesHiddenConversion(Digits, #[tygr(from_str = Semi)] String);

#[derive(Parse)]
struct Nothing;

#[test]
fn hidden_conversion_or_empty_rule_prints_cleanly() {
    assert_eq!(
        rule::<UsesHiddenConversion>(),
        "UsesHiddenConversion = Digits ."
    );
    assert_eq!(rule::<Nothing>(), "Nothing = .");
}

#[derive(Grammar, Debug)]
enum MaybeDigits {
    Digits(Digits),
    Nothing,
}

#[test]
fn empty_alternative_makes_the_rest_optional() {
    assert_eq!(rule::<MaybeDigits>(), "MaybeDigits = [ Digits ] .");
}

#[derive(Grammar, Debug)]
#[tygr(inline)]
struct Inlined(StringEq!("a"), StringEq!("b"));

#[derive(Grammar, Debug)]
struct UsesInlined(Inlined, Digits);

#[test]
fn inline_splices_fields_instead_of_referencing_by_rule() {
    assert_eq!(rule::<UsesInlined>(), "UsesInlined = \"a\" \"b\" Digits .");
    assert_eq!(names::<UsesInlined>(), ["UsesInlined", "Digits"]);
}

#[derive(Grammar, Debug)]
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
        "renamed = \"a\" ^1 .\n^1: be valid"
    );
}

#[derive(Grammar, Debug)]
#[tygr(validated)]
struct NonZero(StringOf1<Digit>);

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
        "NonZero = ( 'digit' { 'digit' } ) ^1 .\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug)]
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
        "ValidatedEnum = ( \"a\" | \"b\" ) ^1 .\n^1: be a recognized token"
    );
}

#[derive(Grammar, Debug)]
#[tygr(inline, validated)]
struct InlineNonZero(StringOf1<Digit>);

impl Validate for InlineNonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        !self.0.starts_with('0')
    }
}

#[derive(Grammar, Debug)]
struct UsesInlineValidated(InlineNonZero, Digits);

#[test]
fn inline_validated_side_condition_propagates_into_the_splice() {
    assert_eq!(
        rule::<UsesInlineValidated>(),
        "UsesInlineValidated = ( 'digit' { 'digit' } ) ^1 Digits .\n^1: be non-zero"
    );
}

#[derive(Grammar, Debug)]
struct UsesInlineValidatedTwice(InlineNonZero, InlineNonZero);

#[test]
fn repeated_inline_validated_side_condition_gets_one_deduplicated_footnote() {
    assert_eq!(
        rule::<UsesInlineValidatedTwice>(),
        "UsesInlineValidatedTwice = ( 'digit' { 'digit' } ) ^1 ( 'digit' { 'digit' } ) ^1 .\n^1: be non-zero"
    );
}

// Field conversions: `#[tygr(<kind> = S)]`.

impl From<Digit> for u8 {
    fn from(d: Digit) -> u8 {
        d.0 as u8 - b'0'
    }
}

impl TryFrom<Digit> for bool {
    type Error = ();
    fn try_from(d: Digit) -> Result<bool, ()> {
        [false, true].get(u8::from(d) as usize).copied().ok_or(())
    }
}

/// `true` as the digit `1`, `false` as `0`.
fn print_flag(flag: &bool, buf: &mut String) {
    buf.push(if *flag { '1' } else { '0' });
}

/// A unit variant's `print_to` takes `&()`.
fn print_null(_: &(), buf: &mut String) {
    buf.push_str("null");
}

#[derive(Debug, PartialEq, Grammar)]
enum Converted {
    #[tygr(try_from_infallible = StringEq!("null"), print_to = print_null)]
    Null,
    Fields(
        #[tygr(from_str = StringOf1<Digit>, print_to = print_display)] u32,
        StringEq!(","),
        #[tygr(from_str_infallible = StringOf1<Digit>, print_to = print_display)] String,
        StringEq!(";"),
        #[tygr(try_from_infallible = Digit, print_to = print_display)] u8,
        #[tygr(try_from = Digit, print_to = print_flag)] bool,
    ),
}

#[test]
fn conversions_parse_and_print_and_only_fallible_ones_get_a_footnote() {
    let Converted::Fields(n, _, text, _, digit, flag) = Converted::parse("42,07;51").unwrap()
    else {
        panic!()
    };
    assert_eq!((n, text.as_str(), digit, flag), (42, "07", 5, true));
    assert_eq!(Converted::parse("null").unwrap(), Converted::Null);
    assert_eq!(Converted::Null.print(), "null");
    assert_eq!(Converted::parse("42,07;51").unwrap().print(), "42,07;51");
    assert_eq!(
        rule::<Converted>(),
        "Converted = \"null\" | ( 'digit' { 'digit' } ) ^1 \",\" 'digit' { 'digit' } \";\" 'digit' 'digit' ^2 .\n^1: be convertible to u32\n^2: be convertible to bool"
    );
    for bad in ["42,07;52", "99999999999,07;51", "nul"] {
        assert!(Converted::parse(bad).is_err() && Converted::check(bad).is_err());
    }
}

#[derive(Debug, Parse)]
#[tygr(validated)]
struct EvenNumber(#[tygr(from_str = StringOf1<Digit>)] u32);

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
        "EvenNumber = ( 'digit' { 'digit' } ) ^1 ^2 .\n^1: be convertible to u32\n^2: be even"
    );
    assert!(EvenNumber::parse("4").is_ok());
    assert!(EvenNumber::parse("3").is_err());
}

#[derive(Debug)]
struct Small<T>(T);

impl TryFrom<Digits> for Small<u8> {
    type Error = std::num::ParseIntError;
    fn try_from(digits: Digits) -> Result<Self, Self::Error> {
        digits.0.parse().map(Small)
    }
}

#[derive(Parse)]
struct Converts(
    #[tygr(try_from = Digits)] Small<u8>,
    StringEq!(","),
    #[tygr(from_str = Digits)] std::num::NonZeroU8,
);

#[test]
fn a_conversion_requires_its_field_type_as_written() {
    assert_eq!(
        rule::<Converts>(),
        "Converts = Digits ^1 \",\" Digits ^2 .\n^1: be convertible to Small<u8>\n^2: be convertible to std::num::NonZeroU8"
    );
    assert!(Converts::parse("7,300").is_err());
}

#[derive(Debug, Parse)]
#[tygr(validated)]
struct NonZeroDigit(#[tygr(try_from_infallible = Digit)] u8);

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
        "NonZeroDigit = 'digit' ^1 .\n^1: be non-zero"
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

#[derive(Debug, Parse)]
struct UsesCounted(#[tygr(from_str_infallible = StringOf1<Digit>)] Counted);

#[test]
fn infallible_conversion_is_skipped_when_checking() {
    let before = CONVERSIONS.load(Relaxed);
    assert!(UsesCounted::check("123").is_ok());
    assert_eq!(CONVERSIONS.load(Relaxed), before);
    assert_eq!(UsesCounted::parse("123").unwrap().0, Counted("123".into()));
    assert_eq!(CONVERSIONS.load(Relaxed), before + 1);
}

/// Parsed only: a `u32` doesn't print.
#[derive(Parse)]
struct ParsedOnly(#[tygr(from_str = StringOf1<Digit>)] u32);

/// Printed only, as when built by hand.
#[derive(Print)]
struct PrintedOnly(StringEq!("#"), Digits);

#[test]
fn parse_and_print_derive_separately() {
    assert_eq!(ParsedOnly::parse("42").unwrap().0, 42);
    let printed = PrintedOnly(Default::default(), Digits(Raw::new("42")));
    assert_eq!(printed.print(), "#42");
}

#[test]
fn values_build_by_hand_to_print() {
    let digits = Vec1::try_from(vec![Digit('4'), Digit('2')]).unwrap();
    assert_eq!((Hidden(digits.clone()), Some(Digit('7'))).print(), "427");
    assert_eq!(<Raw<StringEq!(",")>>::default().as_str(), ",");
    assert_eq!(digits.into_inner().len(), 2);
    assert!(Vec1::<Digit>::try_from(vec![]).is_err());
}

/// Prints `C`, for a unit variant.
fn print_char<const C: char>(_: &(), buf: &mut String) {
    buf.push(C);
}

/// A unit variant's source, converted into `()` by hand.
#[derive(Debug, Grammar)]
struct Keyword(StringEqCI!("kw"));

impl From<Keyword> for () {
    fn from(_: Keyword) {}
}

fn print_keyword(_: &(), buf: &mut String) {
    buf.push_str("kw");
}

#[derive(Debug, PartialEq, Grammar)]
enum Unit {
    #[tygr(try_from = StringEq!("a"), print_to = print_char::<'a'>)]
    A,
    // Case-insensitive, but with one text, so it still converts.
    #[tygr(try_from_infallible = StringEqCI!("1"), print_to = print_char::<'1'>)]
    One,
    #[tygr(try_from_infallible = Keyword, print_to = print_keyword)]
    Kw,
}

#[test]
fn a_unit_variant_converts_its_source_into_unit() {
    for (text, unit) in [("a", Unit::A), ("1", Unit::One), ("kw", Unit::Kw)] {
        assert_eq!(Unit::parse(text).unwrap(), unit);
        assert_eq!(unit.print(), text);
    }
    assert_eq!(Unit::parse("KW").unwrap(), Unit::Kw);
    assert_eq!(
        rule::<Unit>(),
        "Unit = \"a\" ^1 | \"1\" | Keyword .\n^1: be convertible to ()"
    );
}

char_class!(Space, "space", |ch| ch == ' ');

fn print_comma<T>(_: &T, buf: &mut String) {
    buf.push(',');
}

fn print_nothing<T>(_: &T, _: &mut String) {}

/// Drops a required comma and optional spaces; printing them is up to us.
#[derive(Grammar)]
struct Pair(
    Digits,
    #[tygr(print_to = print_comma)] PhantomData<StringEq!(",")>,
    #[tygr(print_to = print_nothing)] PhantomData<StringOf<Space>>,
    Digits,
);

#[test]
fn phantom_data_prints_only_with_print_to() {
    assert_eq!(Pair::parse("1,  2").unwrap().print(), "1,2");
}

#[derive(Grammar)]
enum Never {}

#[test]
fn an_empty_enum_never_parses() {
    assert!(Never::parse("").is_err());
    assert!(<Option<Never>>::parse("").unwrap().is_none());
}

#[derive(Parse)]
struct Num<T: FromStr>(#[tygr(from_str = StringOf1<Digit>)] T);

#[test]
fn a_converted_field_can_be_generic() {
    assert_eq!(Num::<u32>::parse("42").unwrap().0, 42);
    assert_eq!(Num::<i8>::parse("42").unwrap().0, 42);
}
