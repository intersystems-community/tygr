//! JSON parser/printer/EBNF (grammar per json.org), driven from the command line.
//!
//!   cargo run --example json -- bnf
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- parse
//!   echo '{"a": 1}' | cargo run --example json -- test

use std::process::ExitCode;

use either::Either;
use tygr::*;

#[path = "common/mod.rs"]
mod common;

#[derive(Grammar, Debug)]
struct Json(Wrap<Ws, Element, Ws>);

#[derive(Grammar, Debug)]
enum Element {
    Object(Object),
    Array(Array),
    String(String),
    Number(Number),
    True(StringEq!("true")),
    False(StringEq!("false")),
    Null(StringEq!("null")),
}

#[derive(Grammar, Debug)]
struct Object(Wrap<(StringEq!("{"), Ws), Option<Members>, StringEq!("}")>);

#[derive(Grammar, Debug)]
struct Members(VecSep<Suffix<Member, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar, Debug)]
struct Member {
    pub string: String,
    col: Wrap<Ws, StringEq!(":"), Ws>,
    pub value: Element,
}

#[derive(Grammar, Debug)]
struct Array(Wrap<(StringEq!("["), Ws), Option<Elements>, StringEq!("]")>);

#[derive(Grammar, Debug)]
struct Elements(VecSep<Suffix<Element, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar, Debug)]
struct String(Wrap<StringEq!("\""), Characters, StringEq!("\"")>);

#[derive(Grammar, Debug)]
struct Characters(Vec<Character>);

#[derive(Grammar, Debug)]
enum Character {
    Normal(CharOf<NormalCharacter>),
    Escape(Prefix<StringEq!("\\"), Escape>),
}

char_class!(NormalCharacter, "0x20-0x10FFFF except for \" and \\", |c| {
    (0x20..=0x10FFFF).contains(&(c as u32)) && (c != '"') && c != '\\'
});

#[derive(Grammar, Debug)]
enum Escape {
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
    },
}

#[derive(Grammar, Debug)]
enum Hex {
    Digit(Digit),
    UAToF(CharOf<UAToF>),
    LAToF(CharOf<LAToF>),
}

char_class!(UAToF, "A-F", |c| ('A'..='F').contains(&c));
char_class!(LAToF, "a-f", |c| ('a'..='f').contains(&c));

#[derive(Grammar, Debug)]
struct Number {
    integer: Integer,
    fraction: Option<Fraction>,
    exponent: Option<Exponent>,
}

#[derive(Grammar, Debug)]
struct Integer {
    neg: Option<StringEq!("-")>,
    digits: Either<(OneNine, Digits), Digit>,
}

#[derive(Grammar, Debug)]
struct Digits(Vec<Digit>);

#[derive(Grammar, Debug)]
enum Digit {
    Zero(StringEq!("0")),
    OneNine(OneNine),
}

#[derive(Grammar, Debug)]
struct OneNine(CharOf<OneNineChars>);

char_class!(OneNineChars, "1-9", |c| ('1'..='9').contains(&c));

#[derive(Grammar, Debug)]
struct Fraction(Prefix<StringEq!("."), Digits>);

#[derive(Grammar, Debug)]
struct Exponent {
    e: Either<StringEq!("E"), StringEq!("e")>,
    sign: Option<Sign>,
    digits: Digits,
}

#[derive(Grammar, Debug)]
enum Sign {
    Pos(StringEq!("+")),
    Neg(StringEq!("-")),
}

#[derive(Grammar, Debug)]
struct Ws(Vec<W>);

#[derive(Grammar, Debug)]
#[grammar(inline)]
enum W {
    C0020(StringEq!("\u{0020}")),
    C000A(StringEq!("\u{000A}")),
    C000D(StringEq!("\u{000D}")),
    C0009(StringEq!("\u{0009}")),
}

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            println!(
                "{}",
                bnf_rules![
                    Json, Element, Object, Members, Member, Array, Elements, String, Characters,
                    Character, Escape, Hex, Number, Integer, Digits, Digit, Fraction, Exponent,
                    Sign, Ws,
                ]
            );
            ExitCode::SUCCESS
        }
        Some("parse") => common::parse::<Json>(),
        Some("test") => common::test::<Json>(),
        _ => common::usage("json"),
    }
}
