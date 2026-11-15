//! # tygr — TYpe into Grammar Routines
//!
//! Define grammars as Rust types and derive the routine work of grammar
//! handling: a **parser**, a **printer**, and a **presentation** (in EBNF).
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
//! // Parse, or only check, and print back.
//! let e = Expr::parse("1 + 2 * 3").unwrap();
//! assert_eq!(e.print(), "1 + 2 * 3");
//! assert!(Expr::parse_with::<success::Unit, level::Max>("(1 + 2) * 3").is_ok());
//!
//! // A failure says where, and what could have come next.
//! let err = Expr::parse_with::<success::Unit, level::Max>("(1 + 2").unwrap_err();
//! assert_eq!(err.pos(), 6);
//! assert_eq!(
//!     err.to_string(),
//!     "parse error at byte 6, expected:\n\t- \")\"\n\t- 'digit'\n\t- 'space'\n\t- Op1\n\t- Op2"
//! );
//!
//! // The grammar, as EBNF.
//! let bnf: Vec<String> = Expr::bnf_rules().iter().map(ToString::to_string).collect();
//! assert_eq!(bnf.join("\n"), r#"Expr = Expr1 { Op1 Expr1 } .
//! Expr1 = Expr2 { Op2 Expr2 } .
//! Expr2 = "(" Expr ")" | Int .
//! Int = 'digit' { 'digit' } .
//! Op2 = "*" | "/" .
//! Op1 = "+" | "-" ."#);
//! ```
//!
//! ## Grammars as Types
//!
//! Derive [`Grammar`] on a `struct` or `enum` for all three routines, or only
//! [`Parse`] (the parser and EBNF) or [`Print`]. Each supported Rust
//! construct maps to an EBNF concept:
//!
//! | Rust construct | EBNF ([Wirth 1977](https://doi.org/10.1145/359863.359883)) |
//! |----------------|-------------------------------|
//! | `struct`       | sequence (`A B C`)           |
//! | `enum`         | alternation (`A B \| C D`) |
//! | `(A, B, …)`    | inline sequence              |
//! | [`StringEq!`] / [`StringEqCI!`] | a literal (`"if"` / `"if"i`) |
//! | `char`         | any one char (`'any'`)       |
//! | a [`char_class!`] type | one char of the class |
//! | [`StringOf<C>`] / [`StringOf1<C>`] | chars of class `C`, kept as text (`{ C }` / `C { C }`) |
//! | [`Either<A, B>`](either::Either), from the `either` crate | inline alternation (`A \| B`) |
//! | [`Never`] / [`Infallible`](std::convert::Infallible) | the empty alternation (`∅`): never matches, e.g. to switch an alternative off by type parameter |
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
//! | [`Wrap<P, T, S>`] / [`Prefix<P, T>`] / [`Suffix<T, S>`] | sequence shorthands, [`Deref`](std::ops::Deref)-ing to `T` |
//!
//! [`Parse::bnf_rules`] lists a grammar's rules, each once, to sort,
//! [rename](bnf::Rule::rename) and print.
//!
//! Parsing is ordered choice with backtracking by recursive descent, as in
//! PEG:
//! - The first alternative that matches wins: list a literal before its
//!   prefixes (`"<="` before `"<"`).
//! - `[ T ]` and `{ T }` never give back what they matched: `[ "a" ] "ab"`
//!   fails on `ab`.
//! - A left-recursive grammar doesn't compile, as its [`Parse::First`] would
//!   be cyclic.
//! - Input nested deeply enough overflows the stack, which aborts the
//!   process. For untrusted input, limit its size first or parse it on a
//!   thread with a bigger stack.
//!
//! ## Parsing
//!
//! `T::parse(input)` parses the whole `input` into a `T`, or an [`Error`]
//! saying where it failed and what was expected there;
//! [`parse_prefix`](Parse::parse_prefix) parses as much as `T` matches and
//! returns the end too. [`parse_with`](Parse::parse_with)`::<S, L>` and
//! [`parse_prefix_with`](Parse::parse_prefix_with) choose instead:
//!
//! `S`, the [`Success`], is what comes back:
//! - [`success::Value`]: the value parsed.
//! - [`success::Unit`]: only whether it parsed.
//!
//! `L`, the [`Level`], is how much a failure tells:
//! - [`level::Max`] (the default): where it failed and what was expected
//!   there, each in the rules enclosing it ([`Error::traces`]).
//! - [`level::Pos`]: only where ([`Error::pos`]).
//! - [`level::Min`]: only that it failed.
//!
//! [`OLC`] turns a byte offset into a line and character.
//!
//! ## Attributes
//!
//! `#[tygr(...)]` on a derived type:
//! - `name = "..."` — override the EBNF rule name (defaults to the type's
//!   name).
//! - `hidden` — same effect as wrapping in [`Hidden<T>`], but on a whole struct/enum.
//! - `inline` — splice this type's own definition wherever it's
//!   referenced, instead of a rule reference (a recursive one keeps its rule).
//! - `validated` — after a successful parse, run [`Validate::validate`] on the
//!   value; a rejection backtracks as if the grammar hadn't matched.
//! - `unbounded(T, …)` — the derive bounds every type parameter with
//!   [`Parse`] (and [`Print`]); these keep only the bounds you wrote.
//! - `try_from = S` or another conversion below — convert the whole value
//!   from `S`, instead of parsing the fields or variants.
//!
//! ## Conversions
//!
//! `#[tygr(...)]` on a field of type `T`, or on a struct or enum `T` itself,
//! parses another grammar `S` and converts it to `T`, e.g. digits into a
//! `u32`, naming how:
//! - `try_from = S`: `S: TryInto<T>`, on the parsed value.
//! - `try_from_infallible = S`: the same with `Error = Infallible`.
//! - `from_str = S`: `T: FromStr`, on the matched text.
//! - `from_str_infallible = S`: the same with `Err = Infallible`.
//!
//! A failed conversion backtracks, recorded and footnoted in EBNF like a
//! failed [`Validate`] check. Converting loses the text, so to print, the
//! field or type needs `print_to = f`, an `fn(&T, &mut String)` of yours (for
//! a field whose `T` is a grammar, [`Print::print_to`]).
//!
//! A unit variant is a field of type `()`, converted from `S` with:
//! - `try_from_infallible = S`: `S` is a literal with one text (`StringEq!`,
//!   or `StringEqCI!` without cased chars), or a grammar of yours with
//!   `impl From<S> for ()`.
//! - `try_from = S`: the conversion can fail.
//!
//! Its `print_to` takes `&()`.
//!
//! ```
//! # use tygr::*;
//! # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
//! #[derive(Grammar)]
//! struct Port(#[tygr(from_str = StringOf1<Digit>, print_to = print_port)] u16);
//!
//! fn print_port(port: &u16, buf: &mut String) {
//!     buf.push_str(&port.to_string());
//! }
//!
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
//! # use std::str::FromStr;
//! # char_class!(Letter, "letter", |ch| ch.is_ascii_lowercase());
//! # #[derive(Grammar)]
//! # struct Word(StringOf1<Letter>);
//! #[derive(Grammar, Debug, PartialEq)]
//! #[tygr(from_str = Word, print_to = print_answer)]
//! enum Answer {
//!     Yes,
//!     No,
//! }
//!
//! impl FromStr for Answer {
//!     type Err = &'static str;
//!     fn from_str(s: &str) -> Result<Answer, &'static str> {
//!         match s {
//!             "yes" | "y" => Ok(Answer::Yes),
//!             "no" | "n" => Ok(Answer::No),
//!             _ => Err("neither yes nor no"),
//!         }
//!     }
//! }
//!
//! fn print_answer(answer: &Answer, buf: &mut String) {
//!     buf.push_str(if *answer == Answer::Yes { "yes" } else { "no" });
//! }
//!
//! assert_eq!(Answer::parse("y").unwrap(), Answer::Yes);
//! assert_eq!(
//!     Answer::parse("maybe").unwrap_err().to_string(),
//!     "parse error at byte 5, expected:\n\t- 'letter'\n\t- Text at 0..5 must be convertible to Answer: neither yes nor no"
//! );
//! assert_eq!(Answer::No.print(), "no");
//! assert_eq!(
//!     Answer::bnf_rules()[0].to_string(),
//!     "Answer = Word ^1 .\n^1: be convertible to Answer"
//! );
//! ```
//!
//! ## Hand-Written Grammars
//!
//! A leaf or wrapper the derive can't express implements [`Parse`] itself:
//! [`parse_at`](Parse::parse_at), threading a [`State`] and yielding what its
//! [`Success`] asks for, with [`First`](Parse::First) and the two EBNF
//! methods. [`Success`] shows one.
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
    pub use crate::token::{ByteEq, NonAsciiStart, Token, class_parse_at, class_to_bnf};
    pub use either::Either;
}

// Lets derived code inside this crate use `::tygr::` paths.
extern crate self as tygr;

/// Derive [`Parse`](trait@Parse) and [`Print`](trait@Print) together, reading
/// the `#[tygr(...)]` attributes of both.
pub use tygr_derive::Grammar;

/// Derive [`Parse`](trait@Parse) for a `struct`, a sequence of its fields, or
/// an `enum`, an alternation of its variants; or, with a conversion on the
/// type, parse `S` and convert it into the whole value. Optional (see the
/// [crate docs](crate)):
/// - on the type: `#[tygr(name = "…", hidden, inline, validated,
///   unbounded(T))]`, or a conversion;
/// - on a field: `#[tygr(try_from = S)]`, `try_from_infallible`, `from_str`
///   or `from_str_infallible`.
pub use tygr_derive::Parse;

/// Derive [`Print`](trait@Print) for a `struct` or `enum`, printing its
/// fields, or the variant's, in turn. A field or type converted with
/// `#[tygr(try_from = S)]` or another conversion has no grammar to print with,
/// so it needs `#[tygr(print_to = f)]`, an `fn(&T, &mut String)`;
/// `#[tygr(unbounded(T))]` keeps only the bounds you wrote (see the
/// [crate docs](crate)).
pub use tygr_derive::Print;

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
