//! # tygr — TYpe into Grammar Routines
//!
//! Define grammars as Rust types and derive the routine work of grammar
//! handling: a **parser**, a **printer**, and a **presentation** (in EBNF).
//!
//! ## Design
//!
//! Derive [`Grammar`] on a `struct` or `enum` to get a parser, a printer, and
//! a BNF presentation for free, or derive only [`Parse`] (the parser and BNF)
//! or [`Print`]. Each supported Rust construct maps to an EBNF concept.
//!
//! | Rust construct | EBNF ([Wirth 1977](https://doi.org/10.1145/359863.359883)) |
//! |----------------|-------------------------------|
//! | `struct`       | concatenation (`A B C`)      |
//! | `enum`         | alternation (`A B \| C`) |
//! | `(A, B, …)`    | inline concatenation         |
//! | [`Either<A, B>`](either::Either) | inline alternation (`A \| B`) |
//! | [`Vec<T>`]     | repetition (`{ T }`)         |
//! | [`Vec1<T>`]    | one-or-more repetition (`T { T }`)      |
//! | [`Option<T>`]  | optional (`[ T ]`)           |
//! | [`Box<T>`]     | recursive indirection        |
//! | [`NotFollowedBy<G>`] | negative lookahead (`!G`), consumes nothing |
//! | [`FollowedBy<G>`] | positive lookahead (`!!G`), consumes nothing |
//! | [`Hidden<T>`]  | parsed/printed, omitted from BNF |
//! | [`Raw<T>`] | parsed via `T`, kept as the raw matched text |
//! | [`Ranged<T>`] | parsed via `T`, kept alongside its `start..end` byte range |
//! | [`Wrap<L, T, R>`] / [`Prefix<P, T>`] / [`Suffix<T, S>`] | concatenation shorthands, [`Deref`](std::ops::Deref)-ing to `T` |
//! | [`VecSep<T, S>`] | one-or-more repetition with separators (`T { S T }`) |
//!
//! To parse a field as another grammar `S` and convert it, e.g. digits into
//! a `u32`, give `S` in the field's `#[tygr(...)]`, naming the std trait:
//! - `from = S`: `From` the parsed value.
//! - `try_from = S`: `TryFrom` the parsed value.
//! - `from_str = S`: `FromStr` on the matched text.
//! - `from_str_infallible = S`: `FromStr` with `Err = Infallible` on the
//!   matched text.
//!
//! A failed conversion backtracks; an infallible one is skipped when
//! checking. Printing goes back via `S: From<&T>` or `T: Display`. On a
//! unit variant, it matches `S` and keeps nothing (printing `S::default()`).
//!
//! ```
//! # use tygr::*;
//! # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
//! #[derive(Grammar)]
//! struct Port(#[tygr(from_str = StringOf1<IsDigit>)] u16);
//! assert_eq!(Port::parse("8080").unwrap().0, 8080);
//! assert!(Port::parse("99999").is_err());
//! ```
//!
//! `#[tygr(...)]` on a derived type:
//! - `name = "..."` — override the BNF rule name (defaults to the type name).
//! - `hidden` — same effect as wrapping in [`Hidden<T>`], but on a whole struct/enum.
//! - `inline` — splice this type's own definition wherever it's
//!   referenced, instead of a rule reference.
//! - `validated` — after a successful parse, run [`Validate::validate`] on the
//!   value; a rejection backtracks as if the grammar hadn't matched.
//!
//! ## Feature Flags
//!
//! The `tygr` crate enables the `trace_one_node` feature by default.
//! Available features are:
//!
//! | Feature | Description |
//! | ------- | ----------- |
//! | `default` | Enables `trace_one_node`. |
//! | `trace` | Enables all tracing features. |
//! | `trace_pos` | Traces parser positions. |
//! | `trace_one_node` | Traces only the nearest grammar node for each attempt. |
//! | `trace_all_nodes` | Traces the complete grammar node chain for each attempt. |
//! | `lower_bnf_name` | Converts generated BNF names to lowercase. |
//! | `upper_bnf_name` | Converts generated BNF names to uppercase. |
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
//! char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());
//!
//! #[derive(Grammar)]
//! pub struct Int(pub StringOf1<IsDigit>);
//!
//! char_class!(pub IsSpace, "space", |ch| ch.is_ascii_whitespace());
//!
//! #[derive(Grammar)]
//! #[tygr(hidden)]
//! pub struct Ws(pub StringOf<IsSpace>);
//!
//! let e = Expr::parse("1 + 2 * 3").unwrap();
//! assert_eq!(e.print(), "1 + 2 * 3");
//! # #[cfg(not(any(feature = "lower_bnf_name", feature = "upper_bnf_name")))]
//! assert_eq!(
//!     bnf_rules![Expr, Op1, Expr1, Op2, Expr2, Int].to_string(),
//!     "Expr = Expr1 { Op1 Expr1 } .\n\
//! Op1 = \"+\" | \"-\" .\n\
//! Expr1 = Expr2 { Op2 Expr2 } .\n\
//! Op2 = \"*\" | \"/\" .\n\
//! Expr2 = \"(\" Expr \")\" | Int .\n\
//! Int = 'digit' { 'digit' } ."
//! );
//! ```
#![warn(missing_docs)]

pub mod bnf;
mod combinators;
pub mod first;
mod grammar;
mod olc;
mod state;
mod token;
mod wrappers;

pub use combinators::*;
pub use grammar::*;
pub use olc::*;
pub use state::*;
pub use token::{CharClass, StringOf, StringOf1};
pub use wrappers::{Hidden, IntoInner, Ranged, Raw};

// Paths the macros expand to, whatever the user has in scope.
#[doc(hidden)]
pub mod __private {
    pub use crate::token::{
        ByteEq, Literal, NonAsciiStart, Token, class_fail_at, class_parse_at, class_print_to,
        class_to_bnf,
    };
    pub use crate::wrappers::{CONVERTIBLE, from, from_str, from_str_infallible, try_from};
    pub use either::Either;
    pub use tygr_derive::__default_name as default_name;
}

// Lets derived code inside this crate use `::tygr::` paths.
#[doc(hidden)]
extern crate self as tygr;

pub use tygr_derive::{Grammar, Parse, Print};

/// A literal-string grammar: `StringEq!("->")` matches exactly `->`.
///
/// ```
/// # use tygr::*;
/// # char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());
/// # #[derive(Grammar)]
/// # pub struct Num(StringOf1<IsDigit>);
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
/// lower/upper case mappings (`'é'` matches `"É"`, `'ß'` matches `"SS"`).
///
/// ```
/// # use tygr::*;
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// #[derive(Grammar)]
/// pub enum Keyword {
///     Class(StringEqCI!("class")),  // matches "class", "CLASS", "Class", etc.
///     Method(StringEqCI!("method")),
/// }
/// ```
pub use tygr_derive::StringEqCI;

/// Collect BNF rule definitions from one or more [`GrammarRule`] types.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// # #[derive(Grammar)] struct Number(StringOf1<IsDigit>);
/// # #[derive(Grammar)] struct AddOp(StringEq!("+"));
/// # #[derive(Grammar)] struct MulOp(StringEq!("*"));
/// # #[derive(Grammar)] struct Factor(Number);
/// # #[derive(Grammar)] struct Term(Factor);
/// # #[derive(Grammar)] struct Expr(Term);
/// let bnf = bnf_rules![Expr, Term, Factor, Number, AddOp, MulOp];
/// println!("{bnf}");
/// ```
#[macro_export]
macro_rules! bnf_rules {
    ($($T:ty),+ $(,)?) => {{
        let mut _buf = ::std::string::String::new();
        $(
            if !_buf.is_empty() { _buf.push('\n'); }
            _buf.push_str(&<$T as $crate::GrammarRule>::bnf_rule());
        )+
        _buf
    }};
}
