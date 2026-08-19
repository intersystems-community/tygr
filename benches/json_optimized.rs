//! Throughput benchmark for the optimized JSON grammar.
//!
//!   cargo bench --bench json_optimized
//!
//! The grammar is copied from `examples/json_optimized.rs` (minus its CLI).

use std::hint::black_box;
use std::marker::PhantomData;
use std::time::Instant;

use either::Either;
use tygr::*;

#[derive(Grammar)]
struct Json(Wrap<Ws, Element, Ws>);

#[derive(Grammar)]
enum Element {
    Object(Object),
    Array(Array),
    String(Range<PhantomData<String>>),
    Number(Range<PhantomData<Number>>),
    True(StringEq!("true")),
    False(StringEq!("false")),
    Null(StringEq!("null")),
}

#[derive(Grammar)]
struct Object(Wrap<(StringEq!("{"), Ws), Option<Members>, StringEq!("}")>);

#[derive(Grammar)]
struct Members(VecSep<Suffix<Member, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar)]
struct Member {
    pub string: String,
    col: Wrap<Ws, StringEq!(":"), Ws>,
    pub value: Element,
}

#[derive(Grammar)]
struct Array(Wrap<(StringEq!("["), Ws), Option<Elements>, StringEq!("]")>);

#[derive(Grammar)]
struct Elements(VecSep<Suffix<Element, Ws>, Suffix<StringEq!(","), Ws>>);

#[derive(Grammar)]
struct String(Wrap<StringEq!("\""), Characters, StringEq!("\"")>);

#[derive(Grammar)]
struct Characters(Vec<Character>);

#[derive(Grammar)]
enum Character {
    Normal(CharOf<NormalCharacter>),
    Escape(Prefix<StringEq!("\\"), Escape>),
}

char_class!(NormalCharacter, "0x20-0x10FFFF except for \" and \\", |c| {
    (0x20..=0x10FFFF).contains(&(c as u32)) && (c != '"') && c != '\\'
});

#[derive(Grammar)]
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

#[derive(Grammar)]
enum Hex {
    Digit(Digit),
    UAToF(CharOf<UAToF>),
    LAToF(CharOf<LAToF>),
}

char_class!(UAToF, "A-F", |c| ('A'..='F').contains(&c));
char_class!(LAToF, "a-f", |c| ('a'..='f').contains(&c));

#[derive(Grammar)]
struct Number {
    neg: Option<StringEq!("-")>,
    nat: Nat,
    fraction: Option<Fraction>,
    exponent: Option<Exponent>,
}

#[derive(Grammar)]
enum Nat {
    Zero(StringEq!("0")),
    Pos(OneNine, Option<Digits>),
}

#[derive(Grammar)]
struct Digits(Vec<Digit>);

#[derive(Grammar)]
enum Digit {
    Zero(StringEq!("0")),
    OneNine(OneNine),
}

#[derive(Grammar)]
enum OneNine {
    D1(StringEq!("1")),
    D2(StringEq!("2")),
    D3(StringEq!("3")),
    D4(StringEq!("4")),
    D5(StringEq!("5")),
    D6(StringEq!("6")),
    D7(StringEq!("7")),
    D8(StringEq!("8")),
    D9(StringEq!("9")),
}

#[derive(Grammar)]
struct Fraction(Prefix<StringEq!("."), Digits>);

#[derive(Grammar)]
struct Exponent {
    e: Either<StringEq!("E"), StringEq!("e")>,
    sign: Option<Sign>,
    digits: Digits,
}

#[derive(Grammar)]
enum Sign {
    Pos(StringEq!("+")),
    Neg(StringEq!("-")),
}

#[derive(Grammar)]
struct Ws(PhantomData<Vec<W>>);

#[derive(Grammar)]
#[grammar(inline)]
enum W {
    C0020(StringEq!("\u{0020}")),
    C000A(StringEq!("\u{000A}")),
    C000D(StringEq!("\u{000D}")),
    C0009(StringEq!("\u{0009}")),
}

const CANADA: &str = include_str!("../data/canada.json");
const DATA: &str = include_str!("../data/data.json");

fn bench(label: &str, input: &str) {
    let mb = input.len() as f64 / 1e6;
    let mut best = f64::MAX;
    for _ in 0..30 {
        let start = Instant::now();
        let v = Json::parse(input).unwrap();
        best = best.min(start.elapsed().as_secs_f64());
        black_box(&v);
    }
    println!("{label}: {:.3}ms  {:.1} MB/s", best * 1e3, mb / best);
}

fn main() {
    bench("CANADA", CANADA);
    bench("DATA", DATA);
}
