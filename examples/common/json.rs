//! The JSON grammar (per json.org), shared by `examples/json.rs` and
//! `benches/json.rs`. `#[path]`-included; not a standalone target.

use either::Either;
use tygr::*;

#[derive(Grammar, Debug)]
pub(crate) struct Json(Wrap<Ws, Element, Ws>);

#[derive(Grammar, Debug)]
pub(crate) enum Element {
    Object(Object),
    Array(Array),
    String(String),
    Number(Number),
    True(StringEq!("true")),
    False(StringEq!("false")),
    Null(StringEq!("null")),
}

#[derive(Grammar, Debug)]
pub(crate) struct Object(Wrap<(StringEq!("{"), Ws), Option<Members>, StringEq!("}")>);

#[derive(Grammar, Debug)]
pub(crate) struct Members(VecSep<Suffix<Member, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar, Debug)]
pub(crate) struct Member {
    pub string: String,
    col: Wrap<Ws, StringEq!(":"), Ws>,
    pub value: Element,
}

#[derive(Grammar, Debug)]
pub(crate) struct Array(Wrap<(StringEq!("["), Ws), Option<Elements>, StringEq!("]")>);

#[derive(Grammar, Debug)]
pub(crate) struct Elements(VecSep<Suffix<Element, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar, Debug)]
pub(crate) struct String(Wrap<StringEq!("\""), Characters, StringEq!("\"")>);

#[derive(Grammar, Debug)]
pub(crate) struct Characters(Vec<Character>);

#[derive(Grammar, Debug)]
pub(crate) enum Character {
    Normal(NormalCharacter),
    Escape(Prefix<StringEq!("\\"), Escape>),
}

char_class!(pub(crate) NormalCharacter, "0x20-0x10FFFF except for \" and \\", |c| {
    matches!(c as u32, 0x20..=0x10FFFF) && c != '"' && c != '\\'
});

#[derive(Grammar, Debug)]
pub(crate) enum Escape {
    DQ(StringEq!("\"")),
    BS(StringEq!("\\")),
    FS(StringEq!("/")),
    B(StringEq!("b")),
    F(StringEq!("f")),
    N(StringEq!("n")),
    R(StringEq!("r")),
    T(StringEq!("t")),
    U {
        u: StringEq!("u"),
        hex1: Hex,
        hex2: Hex,
        hex3: Hex,
        hex4: Hex,
    },
}

#[derive(Grammar, Debug)]
pub(crate) enum Hex {
    Digit(Digit),
    UAToF(UAToF),
    LAToF(LAToF),
}

char_class!(pub(crate) UAToF, "A-F", |c| matches!(c, 'A'..='F'));
char_class!(pub(crate) LAToF, "a-f", |c| matches!(c, 'a'..='f'));

#[derive(Grammar, Debug)]
pub(crate) struct Number {
    integer: Integer,
    fraction: Option<Fraction>,
    exponent: Option<Exponent>,
}

#[derive(Grammar, Debug)]
pub(crate) struct Integer {
    neg: Option<StringEq!("-")>,
    digits: Either<(OneNine, Digits), Digit>,
}

#[derive(Grammar, Debug)]
pub(crate) struct Digits(Vec1<Digit>);

#[derive(Grammar, Debug)]
pub(crate) enum Digit {
    Zero(StringEq!("0")),
    OneNine(OneNine),
}

#[derive(Grammar, Debug)]
pub(crate) struct OneNine(OneNineChars);

char_class!(pub(crate) OneNineChars, "1-9", |c| matches!(c, '1'..='9'));

#[derive(Grammar, Debug)]
pub(crate) struct Fraction(Prefix<StringEq!("."), Digits>);

#[derive(Grammar, Debug)]
pub(crate) struct Exponent {
    e: Either<StringEq!("E"), StringEq!("e")>,
    sign: Option<Sign>,
    digits: Digits,
}

#[derive(Grammar, Debug)]
pub(crate) enum Sign {
    Pos(StringEq!("+")),
    Neg(StringEq!("-")),
}

#[derive(Grammar, Debug)]
pub(crate) struct Ws(Vec<W>);

#[derive(Grammar, Debug)]
#[grammar(inline)]
pub(crate) enum W {
    C0020(StringEq!("\u{0020}")),
    C000A(StringEq!("\u{000A}")),
    C000D(StringEq!("\u{000D}")),
    C0009(StringEq!("\u{0009}")),
}
