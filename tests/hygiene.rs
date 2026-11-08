//! Derived code must work whatever the user has imported or named.

#[derive(tygr::Grammar)]
struct Kw(tygr::StringEq!("if"));

#[derive(tygr::Grammar)]
enum Op {
    Add(tygr::StringEq!("+")),
    Sub(tygr::StringEq!("-")),
    Mul(tygr::StringEq!("*")),
    Div(tygr::StringEq!("/")),
}

tygr::char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(tygr::Grammar)]
#[tygr(validated)]
struct NonZero(tygr::StringOf1<Digit>);

impl tygr::Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        self.0.bytes().any(|b| b != b'0')
    }
}

#[derive(tygr::Grammar)]
struct Field(
    #[tygr(from_str = tygr::StringOf1<Digit>)] u32,
    tygr::StringEq!(","),
    #[tygr(from_str_infallible = tygr::StringOf1<Digit>)] String,
);

// Fields named like the derived code's own variables.
#[derive(tygr::Grammar)]
struct Locals {
    input: tygr::StringOf1<Digit>,
    pos: tygr::StringEq!(","),
    state: tygr::StringOf1<Digit>,
    mode: tygr::StringEq!(";"),
}

// Generic over an `M`, with enough variants for a dispatch table.
#[derive(tygr::Grammar)]
enum Generic<M> {
    A(M),
    B(tygr::StringEq!("b")),
    C(tygr::StringEq!("c")),
    D(tygr::StringEq!("d")),
}

#[test]
fn derives_compile_and_parse_without_imports() {
    use tygr::Grammar as _;
    assert!(Kw::parse("if").is_ok());
    assert!(Op::parse("*").is_ok() && Op::parse("x").is_err());
    assert!(NonZero::parse("10").is_ok() && NonZero::parse("00").is_err());
    let field = Field::parse("42,7").unwrap();
    assert_eq!((field.0, field.2.as_str()), (42, "7"));
    assert!(Locals::parse("1,2;").is_ok());
    assert!(Generic::<Kw>::parse("if").is_ok() && Generic::<Kw>::scan("d").is_ok());
}
