//! JSON (grammar per json.org): parse it, or print the grammar as EBNF, from
//! the command line.
//!
//!   echo '{"a": [1, true]}' | cargo run --example json -- parse
//!   cargo run --example json -- bnf

use either::Either;
use std::io::{self, Read};
use std::process::ExitCode;
use tygr::*;

#[derive(Grammar, Debug)]
struct Json(Wrap<Ws, JsonValue, Ws>);

#[derive(Grammar, Debug)]
#[tygr(name = "Value")]
enum JsonValue {
    Object(Box<Object>),
    Array(Box<Array>),
    String(JsonString),
    Number(Number),
    True(StringEq!("true")),
    False(StringEq!("false")),
    Null(StringEq!("null")),
}

#[derive(Grammar, Debug)]
struct Object(Wrap<(StringEq!("{"), Ws), Option<Members>, StringEq!("}")>);

#[derive(Grammar, Debug)]
struct Members(
    Suffix<Member, Ws>,
    Vec<(Suffix<StringEq!(","), Ws>, Suffix<Member, Ws>)>,
);

#[derive(Grammar, Debug)]
struct Member {
    string: JsonString,
    colon: Wrap<Ws, StringEq!(":"), Ws>,
    value: JsonValue,
}

#[derive(Grammar, Debug)]
struct Array(Wrap<(StringEq!("["), Ws), Option<Elements>, StringEq!("]")>);

#[derive(Grammar, Debug)]
struct Elements(
    Suffix<JsonValue, Ws>,
    Vec<(Suffix<StringEq!(","), Ws>, Suffix<JsonValue, Ws>)>,
);

#[derive(Grammar, Debug)]
#[tygr(name = "String")]
struct JsonString(Wrap<StringEq!("\""), Characters, StringEq!("\"")>);

#[derive(Grammar, Debug)]
struct Characters(Vec<Character>);

#[derive(Grammar, Debug)]
enum Character {
    Normal(NormalCharacter),
    Escape(Prefix<StringEq!("\\"), Escape>),
}

char_class!(
    NormalCharacter,
    "0x20-0x10FFFF except for \" and \\",
    |ch| ch >= '\u{0020}' && ch != '"' && ch != '\\'
);

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
    U(StringEq!("u"), Hex, Hex, Hex, Hex),
}

#[derive(Grammar, Debug)]
enum Hex {
    Digit(Digit),
    UAToF(UAToF),
    LAToF(LAToF),
}

char_class!(UAToF, "A-F", |ch| matches!(ch, 'A'..='F'));
char_class!(LAToF, "a-f", |ch| matches!(ch, 'a'..='f'));

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
struct Digits(Vec1<Digit>);

#[derive(Grammar, Debug)]
enum Digit {
    Zero(StringEq!("0")),
    OneNine(OneNine),
}

#[derive(Grammar, Debug)]
struct OneNine(OneNineChars);

char_class!(OneNineChars, "1-9", |ch| matches!(ch, '1'..='9'));

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
#[tygr(inline)]
enum W {
    C0020(StringEq!("\u{0020}")),
    C000A(StringEq!("\u{000A}")),
    C000D(StringEq!("\u{000D}")),
    C0009(StringEq!("\u{0009}")),
}

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            for rule in Json::bnf_rules() {
                println!("{rule}");
            }
            ExitCode::SUCCESS
        }
        Some("parse") => {
            report(Json::parse::<Value, level::Max>(&read_stdin()).map(|json| format!("{json:#?}")))
        }
        _ => usage(),
    }
}

/// Read all of stdin, minus the trailing JSON whitespace (`echo` appends a newline).
fn read_stdin() -> String {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");
    input.trim_end_matches([' ', '\t', '\n', '\r']).to_owned()
}

/// Print `shown`, or the error, and exit accordingly.
fn report(shown: Result<String, Error>) -> ExitCode {
    match shown {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Print a usage line naming the available verbs and exit with failure.
fn usage() -> ExitCode {
    eprintln!("usage: json <bnf|parse>");
    eprintln!("  bnf          print the grammar as EBNF");
    eprintln!("  parse        parse stdin, print the AST");
    ExitCode::FAILURE
}
