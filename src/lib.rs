//! # tygr — TYpe into Grammar Routines
//!
//! Define grammars as Rust types and derive the routine work of grammar
//! handling: a **parser**, a **printer**, and a **presentation** (in EBNF).
//!
//! ## Design
//!
//! Derive [`Grammar`] on a `struct` or `enum` for all three, or only
//! [`Parse`] (the parser and EBNF) or [`Print`]. Each supported Rust
//! construct maps to an EBNF concept.
//!
//! | Rust construct | EBNF ([Wirth 1977](https://doi.org/10.1145/359863.359883)) |
//! |----------------|-------------------------------|
//! | `struct`       | sequence (`A B C`)           |
//! | `enum`         | alternation (`A B \| C D`) |
//! | `(A, B, …)`    | inline sequence              |
//! | `()`           | the empty sequence           |
//! | [`StringEq!`] / [`StringEqCI!`] | a literal (`"if"` / `"if"i`) |
//! | `char`         | any one char (`'any'`)       |
//! | a [`char_class!`] type | one char of the class (`'digit'`) |
//! | [`StringOf<C>`] / [`StringOf1<C>`] | chars of class `C`, kept as text (`{ 'digit' }` / `'digit' { 'digit' }`) |
//! | [`Either<A, B>`](either::Either), from the `either` crate | inline alternation (`A \| B`) |
//! | [`Vec<T>`]     | repetition (`{ T }`)         |
//! | [`Vec1<T>`]    | one-or-more repetition (`T { T }`)      |
//! | [`Option<T>`]  | optional (`[ T ]`)           |
//! | [`Box<T>`]     | recursive indirection        |
//! | [`NotFollowedBy<G>`] | negative lookahead (`!G`), consumes nothing |
//! | [`FollowedBy<G>`] | positive lookahead (`!!G`), consumes nothing |
//! | [`Hidden<T>`]  | parsed/printed, omitted from EBNF |
//! | [`Raw<T>`] | parsed via `T`, kept as the raw matched text |
//! | [`Ranged<T>`] | parsed via `T`, kept alongside its `start..end` byte range |
//! | [`PhantomData<T>`](std::marker::PhantomData) | parsed via `T`, kept as nothing; doesn't print |
//! | [`Wrap<L, T, R>`] / [`Prefix<P, T>`] / [`Suffix<T, S>`] | sequence shorthands, [`Deref`](std::ops::Deref)-ing to `T` |
//!
//! [`Parse::bnf_rules`] lists a grammar's rules, each once, to sort,
//! [rename](bnf::Rule::rename) and print.
//!
//! Parsing is ordered choice with backtracking, as in PEG: the first
//! alternative that matches wins, so list a literal before its prefixes
//! (`"<="` before `"<"`); likewise `[ T ]` and `{ T }` never give back what
//! they matched, so `[ "a" ] "ab"` fails on `ab`. A left-recursive grammar
//! doesn't compile, as its [`Parse::First`] would be cyclic.
//!
//! A parse that fails runs a second time, in [`Record`] mode, to record what
//! it expected at the furthest position it reached ([`Error::traces`]): a
//! success costs no tracing, a failure about one and a half parses.
//!
//! Parsing recurses as the grammar nests, so input nested deeply enough
//! overflows the stack, which aborts the process. For untrusted input,
//! limit its size first or parse it on a thread with a bigger stack.
//!
//! `#[tygr(...)]` on a derived type:
//! - `name = "..."` — override the EBNF rule name (defaults to the type's
//!   name, which a generic type's instantiations share, so mark such a type
//!   `inline`).
//! - `hidden` — same effect as wrapping in [`Hidden<T>`], but on a whole struct/enum.
//! - `inline` — splice this type's own definition wherever it's
//!   referenced, instead of a rule reference (a recursive one keeps its rule).
//! - `validated` — after a successful parse, run [`Validate::validate`] on the
//!   value; a rejection backtracks as if the grammar hadn't matched.
//! - `unbounded(T, …)` — the derive bounds every type parameter with
//!   [`Parse`] (and [`Print`]); these keep only the bounds you wrote, as a
//!   conversion target or a `D` whose `D::Kw` is the grammar must.
//! - `try_from = S` or another conversion below — convert the whole value
//!   from `S`, instead of parsing the fields or variants.
//!
//! `#[tygr(...)]` on a field of type `T`, or on a struct or enum `T` itself,
//! parses another grammar `S` and converts it to `T`, e.g. digits into a
//! `u32`, naming how:
//! - `try_from = S`: `S: TryInto<T>`, on the parsed value.
//! - `try_from_infallible = S`: the same with `Error = Infallible`, as for
//!   every `From` conversion.
//! - `from_str = S`: `T: FromStr`, on the matched text.
//! - `from_str_infallible = S`: the same with `Err = Infallible`.
//!
//! A failed conversion backtracks, recorded and footnoted in EBNF like a
//! failed [`Validate`] check; an infallible one is skipped when checking.
//! Converting loses the text, so to print, the field or type needs
//! `print_to = f`, an `fn(&T, &mut String)` such as [`print_display`] (or,
//! for a field whose `T` is a grammar, [`Print::print_to`]). A unit variant
//! is a field of type `()`: `try_from_infallible = S` converts `S` into `()`,
//! which a literal with one text does (`StringEq!`, or `StringEqCI!` without
//! cased chars), as does a grammar of yours with `impl From<S> for ()`;
//! `try_from` if the conversion can fail. Its `print_to` takes `&()`.
//!
//! ```
//! # use tygr::*;
//! # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
//! #[derive(Grammar)]
//! struct Port(#[tygr(from_str = StringOf1<Digit>, print_to = print_display)] u16);
//! assert_eq!(Port::parse("8080").unwrap().0, 8080);
//! assert!(Port::parse("99999").is_err());
//! assert_eq!(Port(443).print(), "443");
//! ```
//!
//! Convert a field when the type around it is the grammar, as `Port` is.
//! Convert the type when it is built from a grammar rather than made of
//! grammars, as an enum of plain variants or a struct of data fields is: its
//! fields or variants are then yours to fill, not parts of the grammar, and
//! take no `#[tygr(...)]`.
//!
//! ```
//! # use tygr::*;
//! # use std::{fmt, str::FromStr};
//! # char_class!(Letter, "letter", |ch| ch.is_ascii_lowercase());
//! # #[derive(Grammar)]
//! # struct Word(StringOf1<Letter>);
//! #[derive(Grammar, Debug, PartialEq)]
//! #[tygr(from_str = Word, print_to = print_display)]
//! enum Answer {
//!     Yes,
//!     No,
//! }
//!
//! impl FromStr for Answer {
//!     type Err = ();
//!     fn from_str(s: &str) -> Result<Answer, ()> {
//!         match s {
//!             "yes" | "y" => Ok(Answer::Yes),
//!             "no" | "n" => Ok(Answer::No),
//!             _ => Err(()),
//!         }
//!     }
//! }
//!
//! impl fmt::Display for Answer {
//!     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//!         f.write_str(if *self == Answer::Yes { "yes" } else { "no" })
//!     }
//! }
//!
//! assert_eq!(Answer::parse("y").unwrap(), Answer::Yes);
//! assert!(Answer::parse("maybe").is_err());
//! assert_eq!(Answer::No.print(), "no");
//! assert_eq!(
//!     Answer::bnf_rules()[0].to_string(),
//!     "Answer = Word ^1 .\n^1: be convertible to Answer"
//! );
//! ```
//!
//! ## Quick Example
//!
//! ```rust
//! use tygr::*;
//!
//! #[derive(Grammar)]
//! pub struct Expr(pub Expr1, pub Vec<(Wrap<Ws, Op1, Ws>, Expr1)>);
//!
//! #[derive(Grammar)]
//! pub enum Op1 {
//!     Add(StringEq!("+")),
//!     Sub(StringEq!("-")),
//! }
//!
//! #[derive(Grammar)]
//! pub struct Expr1(pub Expr2, pub Vec<(Wrap<Ws, Op2, Ws>, Expr2)>);
//!
//! #[derive(Grammar)]
//! pub enum Op2 {
//!     Mul(StringEq!("*")),
//!     Div(StringEq!("/")),
//! }
//!
//! #[derive(Grammar)]
//! pub enum Expr2 {
//!     Paren(Wrap<(StringEq!("("), Ws), Box<Expr>, (Ws, StringEq!(")"))>),
//!     Number(Int),
//! }
//!
//! char_class!(pub Digit, "digit", |ch| ch.is_ascii_digit());
//!
//! #[derive(Grammar)]
//! pub struct Int(pub StringOf1<Digit>);
//!
//! char_class!(pub Space, "space", |ch| ch.is_ascii_whitespace());
//!
//! #[derive(Grammar)]
//! #[tygr(hidden)]
//! pub struct Ws(pub StringOf<Space>);
//!
//! let e = Expr::parse("1 + 2 * 3").unwrap();
//! assert_eq!(e.print(), "1 + 2 * 3");
//!
//! let bnf: Vec<String> = Expr::bnf_rules().iter().map(ToString::to_string).collect();
//! assert_eq!(bnf.join("\n"), r#"Expr = Expr1 { Op1 Expr1 } .
//! Expr1 = Expr2 { Op2 Expr2 } .
//! Expr2 = "(" Expr ")" | Int .
//! Int = 'digit' { 'digit' } .
//! Op2 = "*" | "/" .
//! Op1 = "+" | "-" ."#);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod bnf;
mod combinators;
mod convert;
pub mod first;
mod grammar;
mod olc;
#[cfg(doctest)]
mod rejected;
mod state;
mod token;
mod wrappers;

pub use combinators::*;
pub use grammar::*;
pub use olc::*;
pub use state::*;
pub use token::{CharClass, StringOf, StringOf1};
pub use wrappers::*;

// Paths the macros expand to, whatever the user has in scope.
#[doc(hidden)]
pub mod __private {
    pub use crate::bnf::{inline_bnf, visit_inline};
    pub use crate::convert::{from_str, from_str_infallible, try_from, try_from_infallible};
    pub use crate::token::{
        ByteEq, NonAsciiStart, Token, class_fail_at, class_parse_at, class_to_bnf,
    };
    pub use either::Either;
}

// Lets derived code inside this crate use `::tygr::` paths.
extern crate self as tygr;

pub use tygr_derive::{Grammar, Parse, Print};

/// A literal-string grammar: `StringEq!("->")` matches exactly `->`, and is
/// `Default`, to build one by hand (e.g. to print).
/// In a generic type, std's derives (`Debug`, `Clone`, …) reject a type macro
/// as a field: alias it first, `type Arrow = StringEq!("->");`.
///
/// ```
/// # use tygr::*;
/// # char_class!(pub Digit, "digit", |ch| ch.is_ascii_digit());
/// # #[derive(Grammar)]
/// # pub struct Num(StringOf1<Digit>);
/// # #[derive(Grammar)]
/// # pub struct Expr(Num);
/// #[derive(Grammar)]
/// pub enum Factor {
///     Paren(StringEq!("("), Box<Expr>, StringEq!(")")),
///     Num(Num),
/// }
/// ```
pub use tygr_derive::StringEq;

/// A case-insensitive [`StringEq!`]: each char also matches its Unicode
/// lower/upper case mappings and theirs (`'é'` matches `"É"`, `'ß'` matches
/// `"SS"` and `"ss"`). The value keeps the spelling it matched, so it prints
/// back as is and `"IF"` and `"if"` parse to unequal values.
///
/// ```
/// # use tygr::*;
/// #[derive(Grammar)]
/// pub enum Keyword {
///     Class(StringEqCI!("class")),  // matches "class", "CLASS", "Class", etc.
///     Method(StringEqCI!("method")),
/// }
/// ```
pub use tygr_derive::StringEqCI;
