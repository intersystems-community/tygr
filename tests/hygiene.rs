//! Derived code must work whatever the user has imported or named, and in
//! crates that forbid `unsafe`.
#![forbid(unsafe_code)]

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
    #[tygr(from_str = tygr::StringOf1<Digit>, print_to = tygr::print_display)] u32,
    tygr::StringEq!(","),
    #[tygr(try_from = Kw, print_to = print_kw)] Keyword,
);

struct Keyword;

impl TryFrom<Kw> for Keyword {
    type Error = ();
    fn try_from(_: Kw) -> Result<Keyword, ()> {
        Ok(Keyword)
    }
}

fn print_kw(_: &Keyword, buf: &mut String) {
    buf.push_str("if");
}

// Fields named like the derived code's own variables.
#[derive(tygr::Grammar)]
struct Locals {
    input: tygr::StringOf1<Digit>,
    pos: tygr::StringEq!(","),
    state: tygr::StringOf1<Digit>,
    mode: tygr::StringEq!(";"),
    buf: tygr::StringEq!("."),
    rules: tygr::StringEq!("!"),
}

// Generic over an `M`, with enough variants for a dispatch table.
#[derive(tygr::Grammar)]
enum Generic<M> {
    A(M),
    B(tygr::StringEq!("b")),
    C(tygr::StringEq!("c")),
    D(tygr::StringEq!("d")),
}

// No prelude at all, and items named like the prelude's.
mod bare {
    #![no_implicit_prelude]
    #![allow(dead_code)]

    struct Option;
    struct Some;
    struct None;
    struct Vec;
    // Never called: `char_class!` must not pick it up for its default name.
    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($t:tt)*) => {
            "shadowed"
        };
    }

    ::tygr::char_class!(pub(super) Digit, |ch| ch.is_ascii_digit());

    #[derive(::tygr::Grammar)]
    pub(super) enum Op {
        Add(::tygr::StringEq!("+")),
        Sub(::tygr::StringEq!("-")),
        Mul(::tygr::StringEq!("*")),
        Div(::tygr::StringEq!("/")),
    }

    #[derive(::tygr::Grammar)]
    #[tygr(validated)]
    pub(super) struct NonZero(
        #[tygr(from_str = ::tygr::StringOf1<Digit>, print_to = ::tygr::print_display)] u32,
    );

    impl ::tygr::Validate for NonZero {
        const REQUIREMENT: &'static str = "be non-zero";
        fn validate(&self) -> bool {
            self.0 != 0
        }
    }
}

#[test]
fn derives_work_without_a_prelude() {
    use tygr::{CharClass as _, Parse as _, Print as _};
    assert!(bare::Op::parse("*").is_ok() && bare::Op::parse("x").is_err());
    assert_eq!(bare::NonZero::parse("42").unwrap().print(), "42");
    assert!(bare::NonZero::parse("0").is_err());
    assert_eq!(bare::Digit::name(), "Digit");
}

#[test]
fn derives_compile_and_parse_without_imports() {
    use tygr::Parse as _;
    assert!(Kw::parse("if").is_ok());
    assert!(Op::parse("*").is_ok() && Op::parse("x").is_err());
    assert!(NonZero::parse("10").is_ok() && NonZero::parse("00").is_err());
    let field = Field::parse("42,if").unwrap();
    assert_eq!(field.0, 42);
    assert_eq!(tygr::Print::print(&field), "42,if");
    let locals = Locals::parse("1,2;.!").unwrap();
    assert_eq!(tygr::Print::print(&locals), "1,2;.!");
    assert_eq!(Locals::bnf_rules().len(), 1);
    assert!(Generic::<Kw>::parse("if").is_ok() && Generic::<Kw>::check("d").is_ok());
}
