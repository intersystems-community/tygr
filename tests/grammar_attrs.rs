//! `#[tygr(...)]` options on types and fields, and the EBNF they give.

use std::fmt;
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
    type Error = &'static str;
    fn try_from(d: Digit) -> Result<bool, &'static str> {
        [false, true]
            .get(u8::from(d) as usize)
            .copied()
            .ok_or("be 0 or 1")
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

/// Counts its conversions, converting on the type as well as in a field.
#[derive(Debug, PartialEq, Parse)]
#[tygr(from_str_infallible = StringOf1<Digit>)]
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
    assert!(UsesCounted::check("123").is_ok() && Counted::check("123").is_ok());
    assert_eq!(CONVERSIONS.load(Relaxed), before);
    assert_eq!(UsesCounted::parse("123").unwrap().0, Counted("123".into()));
    assert_eq!(Counted::parse("123").unwrap(), Counted("123".into()));
    assert_eq!(CONVERSIONS.load(Relaxed), before + 2);
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

/// Every combinator is `Hash`, so a grammar holding them all can derive it.
#[derive(Grammar, PartialEq, Eq, Hash)]
struct Hashable(
    Wrap<StringEq!("("), Vec1<Digit>, StringEq!(")")>,
    Prefix<StringEq!("#"), Ranged<Digit>>,
    Suffix<(Digit, Vec<(StringEq!(","), Digit)>), StringEq!(";")>,
    NotFollowedBy<StringEq!("!")>,
    FollowedBy<Digit>,
    Raw<Digit>,
    Hidden<StringOf<Digit>>,
);

#[test]
fn a_grammar_of_combinators_derives_hash() {
    let parse = || Hashable::parse("(1)#23,4;56").unwrap();
    assert_eq!(std::collections::HashSet::from([parse(), parse()]).len(), 1);
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

#[derive(Grammar)]
#[tygr(unbounded(T))]
struct Num<T: FromStr<Err: std::fmt::Display> + std::fmt::Display>(
    #[tygr(from_str = StringOf1<Digit>, print_to = print_display)] T,
);

#[test]
fn a_converted_field_can_be_generic() {
    assert_eq!(Num::<u32>::parse("42").unwrap().0, 42);
    assert_eq!(Num::<i8>::parse("42").unwrap().print(), "42");
}

trait Dialect {
    type Kw: Parse + Print;
}
struct En;
impl Dialect for En {
    type Kw = StringEq!("if");
}

/// `D` carries the grammar `D::Kw`, so it stays unbounded.
#[derive(Grammar)]
#[tygr(unbounded(D))]
struct Stmt<D: Dialect>(D::Kw, Digits);

#[test]
fn an_unbounded_parameter_keeps_only_its_own_bounds() {
    assert_eq!(Stmt::<En>::parse("if1").unwrap().print(), "if1");
    assert!(Stmt::<En>::parse("if").is_err());
}

// Conversions on the type: `#[tygr(<kind> = S)]` on a struct or enum
// converts `S` into the whole value; its fields and variants aren't parsed.

char_class!(Lower, "lower", |ch| ch.is_ascii_lowercase());

#[derive(Grammar, Debug)]
struct Word(StringOf1<Lower>);

#[derive(Debug, PartialEq, Grammar)]
#[tygr(try_from = Word, print_to = print_color)]
enum Color {
    Red,
    Green,
    Blue,
}

impl TryFrom<Word> for Color {
    type Error = &'static str;
    fn try_from(word: Word) -> Result<Color, &'static str> {
        match word.0.as_str() {
            "red" => Ok(Color::Red),
            "green" => Ok(Color::Green),
            "blue" => Ok(Color::Blue),
            _ => Err("name a color"),
        }
    }
}

fn print_color(color: &Color, buf: &mut String) {
    buf.push_str(match color {
        Color::Red => "red",
        Color::Green => "green",
        Color::Blue => "blue",
    });
}

#[test]
fn an_enum_converts_its_source_into_the_whole_value() {
    assert_eq!(Color::parse("red").unwrap(), Color::Red);
    assert_eq!(Color::Blue.print(), "blue");
    assert!(Color::parse("pink").is_err() && Color::check("pink").is_err());
    assert_eq!(
        rule::<Color>(),
        "Color = Word ^1 .\n^1: be convertible to Color"
    );
}

#[derive(Debug, PartialEq, Grammar)]
#[tygr(from_str = Digits, print_to = print_display)]
struct Port(u16);

impl FromStr for Port {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Port, Self::Err> {
        s.parse().map(Port)
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Built from a sequence, kept as numbers: infallible, so no footnote.
#[derive(Debug, PartialEq, Grammar)]
#[tygr(try_from_infallible = (Digits, StringEq!(","), Digits), print_to = print_point)]
struct Point {
    x: u32,
    y: u32,
}

impl From<(Digits, StringEq!(","), Digits)> for Point {
    fn from((x, _, y): (Digits, StringEq!(","), Digits)) -> Point {
        let number = |digits: Digits| digits.0.parse().unwrap();
        Point {
            x: number(x),
            y: number(y),
        }
    }
}

fn print_point(point: &Point, buf: &mut String) {
    buf.push_str(&format!("{},{}", point.x, point.y));
}

#[test]
fn a_struct_converts_from_its_source_or_its_text() {
    assert_eq!(Port::parse("8080").unwrap(), Port(8080));
    assert!(Port::parse("99999").is_err());
    assert_eq!(Port(443).print(), "443");
    assert_eq!(
        rule::<Port>(),
        "Port = Digits ^1 .\n^1: be convertible to Port"
    );
    assert_eq!(Point::parse("1,2").unwrap(), Point { x: 1, y: 2 });
    assert_eq!(Point { x: 3, y: 4 }.print(), "3,4");
    assert_eq!(rule::<Point>(), "Point = Digits \",\" Digits .");
}

/// Used as a field, as the wrapper it replaces was.
#[derive(Debug, Grammar)]
struct Colors(Color, StringEq!(","), Color);

#[test]
fn a_converted_type_is_a_rule_other_rules_refer_to() {
    assert_eq!(Colors::parse("red,blue").unwrap().print(), "red,blue");
    assert!(Colors::parse("red,pink").is_err());
    let rules: Vec<String> = Colors::bnf_rules()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        rules,
        [
            "Colors = Color \",\" Color .",
            "Color = Word ^1 .\n^1: be convertible to Color",
            "Word = 'lower' { 'lower' } .",
        ]
    );
}

/// The requirement names the Rust type, whatever the rule is named.
#[derive(Debug, Parse)]
#[tygr(name = "even", from_str = StringOf1<Digit>, validated)]
struct Even(u32);

impl FromStr for Even {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Even, Self::Err> {
        s.parse().map(Even)
    }
}

impl Validate for Even {
    const REQUIREMENT: &'static str = "be even";
    fn validate(&self) -> bool {
        self.0.is_multiple_of(2)
    }
}

#[test]
fn a_converted_type_is_validated_after_converting() {
    assert_eq!(
        rule::<Even>(),
        "even = ( 'digit' { 'digit' } ) ^1 ^2 .\n^1: be convertible to Even\n^2: be even"
    );
    assert_eq!(Even::parse("4").unwrap().0, 4);
    assert!(Even::parse("3").is_err() && Even::check("3").is_err());
}

/// A converted type's generic parameter is no grammar either.
#[derive(Debug, Grammar)]
#[tygr(unbounded(T), from_str = StringOf1<Digit>, print_to = print_display)]
struct Parsed<T: FromStr<Err: fmt::Display> + fmt::Display>(T);

impl<T: FromStr<Err: fmt::Display> + fmt::Display> FromStr for Parsed<T> {
    type Err = T::Err;
    fn from_str(s: &str) -> Result<Self, T::Err> {
        s.parse().map(Parsed)
    }
}

impl<T: FromStr<Err: fmt::Display> + fmt::Display> fmt::Display for Parsed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[test]
fn a_converted_type_can_be_generic() {
    assert_eq!(Parsed::<u8>::parse("42").unwrap().print(), "42");
    assert!(Parsed::<u8>::parse("300").is_err());
    assert_eq!(
        rule::<Parsed<i8>>(),
        "Parsed = ( 'digit' { 'digit' } ) ^1 .\n^1: be convertible to Parsed<T>"
    );
}

#[derive(Debug, Parse)]
#[tygr(inline, try_from = Word)]
struct InlineColor(Color);

impl TryFrom<Word> for InlineColor {
    type Error = &'static str;
    fn try_from(word: Word) -> Result<Self, &'static str> {
        Color::try_from(word).map(InlineColor)
    }
}

#[derive(Debug, Parse)]
#[tygr(hidden, try_from_infallible = Word)]
struct HiddenWord(String);

impl From<Word> for HiddenWord {
    fn from(word: Word) -> Self {
        HiddenWord(word.0.into_inner())
    }
}

#[derive(Debug, Parse)]
struct UsesConvertedTypes(InlineColor, StringEq!(":"), HiddenWord);

#[test]
fn a_converted_type_can_be_inline_or_hidden() {
    let value = UsesConvertedTypes::parse("red:x").unwrap();
    assert_eq!((value.0.0, value.2.0.as_str()), (Color::Red, "x"));
    assert_eq!(
        rule::<UsesConvertedTypes>(),
        "UsesConvertedTypes = Word ^1 \":\" .\n^1: be convertible to InlineColor"
    );
    assert_eq!(
        names::<UsesConvertedTypes>(),
        ["UsesConvertedTypes", "Word"]
    );
    assert!(HiddenWord::bnf_rules().is_empty());
}

/// A unit struct from a literal, as a unit variant converts one into `()`.
#[derive(Debug, PartialEq, Grammar)]
#[tygr(try_from_infallible = StringEq!("null"), print_to = print_null_struct)]
struct NullStruct;

impl From<StringEq!("null")> for NullStruct {
    fn from(_: StringEq!("null")) -> Self {
        NullStruct
    }
}

fn print_null_struct(_: &NullStruct, buf: &mut String) {
    buf.push_str("null");
}

#[test]
fn a_unit_struct_converts_from_a_literal() {
    assert_eq!(NullStruct::parse("null").unwrap(), NullStruct);
    assert_eq!(NullStruct.print(), "null");
    assert_eq!(rule::<NullStruct>(), "NullStruct = \"null\" .");
}
