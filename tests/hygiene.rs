//! Derived code must not rely on anything the user has imported.

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
#[grammar(validated)]
struct NonZero(tygr::StringOf1<Digit>);

impl tygr::Validate for NonZero {
    const REQUIREMENT: &'static str = "be non-zero";
    fn validate(&self) -> bool {
        self.0.bytes().any(|b| b != b'0')
    }
}

#[derive(tygr::GrammarFromStr)]
struct Num(u32);

impl tygr::GrammarFrom for Num {
    type Source = tygr::StringOf1<Digit>;
    fn print_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self.0.to_string().as_bytes());
    }
}

impl std::str::FromStr for Num {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Num)
    }
}

#[test]
fn derives_compile_and_parse_without_imports() {
    use tygr::Grammar as _;
    assert!(Kw::parse("if").is_ok());
    assert!(Op::parse("*").is_ok() && Op::parse("x").is_err());
    assert!(NonZero::parse("10").is_ok() && NonZero::parse("00").is_err());
    assert_eq!(Num::parse("42").unwrap().0, 42);
}
