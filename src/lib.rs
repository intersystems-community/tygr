//! # tygr — TYpe into Grammar Routines
//!
//! Define grammars as Rust types and derive the routine work of grammar
//! handling: a **parser**, a **printer**, and a **presentation** (in EBNF).
//!
//! ## Design
//!
//! Derive [`Grammar`] on a `struct` or `enum` to get a parser, a printer, and
//! a BNF presentation for free. Each supported Rust construct maps to an
//! EBNF concept.
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
//! | [`Hidden<T>`]  | parsed/printed, omitted from BNF |
//! | [`Raw<T>`]     | parsed via `T`, kept as the raw matched text |
//! | [`Range<T>`]   | parsed via `T`, kept alongside its `[start, end)` span |
//! | [`Wrap<L, T, R>`] / [`Prefix<P, T>`] / [`Suffix<T, S>`] | concatenation shorthands, [`Deref`]-ing to `T` |
//! | [`VecSep<T, S>`] | one-or-more repetition with separators (`T { S T }`) |
//!
//! For a type that isn't itself a direct concatenation/alternation of other
//! grammars, but is instead built *from* one (e.g. parsing digits into a
//! `u32`), derive [`GrammarFromStr`](derive@GrammarFromStr),
//! [`GrammarFromOther`](derive@GrammarFromOther), or
//! [`GrammarTryFromOther`](derive@GrammarTryFromOther) instead — each builds
//! `Self` from a parsed [`GrammarFrom::Source`] via `FromStr`, `From`, or
//! `TryFrom` respectively.
//!
//! `#[grammar(...)]` on a `#[derive(Grammar)]` type:
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
//! #[grammar(hidden)]
//! pub struct Ws(pub StringOf<IsSpace>);
//!
//! let e = Expr::parse("1 + 2 * 3").unwrap();
//! assert_eq!(e.print(), "1 + 2 * 3");
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
mod char;
mod grammar;
mod olc;
mod state;

use std::ops::Deref;

pub use char::*;
pub use grammar::*;
pub use olc::*;
pub use state::*;

// Re-exported so the `StringEq!()`/`StringEqCI!()` macro expansions can
// reference them, but hidden from docs so users always reach for the macros
// instead of naming these types directly. `StringEq`/`StringEqCI` here are
// types (in the type namespace); the macros of the same name below live in
// the macro namespace, so there's no conflict.
#[doc(hidden)]
pub use char::{CharCIThen, CharThen, StringEq, StringEqCI};

// Lets generated code inside this crate use `::tygr::...` paths, matching
// how downstream crates refer to it.
#[doc(hidden)]
extern crate self as tygr;

pub use tygr_derive::Grammar;
pub use tygr_derive::{GrammarFromOther, GrammarFromStr, GrammarTryFromOther};

/// Expand a string literal into a literal-token type.
///
/// - Single character: `StringEq!(",")` → `StringEq<CharThen<',', ()>>`
/// - Multiple characters: `StringEq!("->")` → `StringEq<CharThen<'-', CharThen<'>', ()>>>`
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

/// Expand a string literal into a *case-insensitive* literal-token type.
///
/// Like [`StringEq!`], but matches both uppercase and lowercase variants.
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

// ── Macros ──────────────────────────────────────────────────────────────────

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

/// Define a [`CharClass`] in one line.
///
/// Three forms:
///
/// ```
/// # use tygr::*;
/// char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// ```
///
/// A visibility qualifier is optional (defaults to private):
///
/// ```
/// # use tygr::*;
/// char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());
/// ```
///
/// ```
/// # use tygr::*;
/// char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
///
/// #[derive(Grammar)]
/// struct Num(StringOf1<IsDigit>);
/// ```
#[macro_export]
macro_rules! char_class {
    ($vis:vis $name:ident, $label:expr, |$ch:ident| $pred:expr) => {
        $vis struct $name;
        impl $crate::CharClass for $name {
            fn matches($ch: char) -> bool { $pred }
            fn name() -> &'static str { $label }
        }
    };
}

/// Return type of [`Validate::validate`] — either `bool` (`false` rejects) or
/// `Option<&'static str>` (`Some(msg)` rejects with a reason).
pub trait Validation {
    /// `None` accepts; `Some(msg)` rejects, with `msg` describing what was expected.
    fn be_valid(self) -> Option<&'static str>;
}

impl Validation for bool {
    fn be_valid(self) -> Option<&'static str> {
        if self { None } else { Some("be valid") }
    }
}

impl Validation for Option<&'static str> {
    fn be_valid(self) -> Option<&'static str> {
        self
    }
}

/// Post-parse validation for `#[grammar(validated)]` types.
///
/// Return `false`/`None` to reject the parsed value (the parse backtracks as
/// if the grammar hadn't matched); `true`/`Some(msg)` accepts it, where `msg`
/// with `Some` is reported in traces as "must {msg}".
pub trait Validate {
    /// See [`Validation`].
    type Result: Validation;

    /// Check the just-parsed value; see the trait docs for how the result is interpreted.
    fn validate(&self) -> Self::Result;
}

// `pub`, not private: leaks into `VecSep`'s public `Grammar::First` associated
// type through the `rest` field, so it must stay nameable (E0446).
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
#[doc(hidden)]
pub struct SepItem<S, T> {
    sep: S,
    item: T,
}

/// One or more items, with a separator between each pair.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct VecSep<T, S> {
    head: Box<T>,
    rest: Vec<SepItem<S, T>>,
}

impl<T, S> VecSep<T, S> {
    /// Iterate over the `T` items, in order.
    pub fn items(&self) -> impl Iterator<Item = &T> {
        vec![self.head.as_ref()]
            .into_iter()
            .chain(self.rest.iter().map(|sep_item| &sep_item.item))
    }

    /// Iterate over the `S` separators, in order.
    pub fn seps(&self) -> impl Iterator<Item = &S> {
        self.rest.iter().map(|sep_item| &sep_item.sep)
    }

    /// Consume into `(item, separator)` pairs, one per separator (the head
    /// item is paired with the separator that follows it).
    pub fn into_item_sep(self) -> impl Iterator<Item = (T, S)> {
        self.rest.into_iter().scan(*self.head, |prev, sep_item| {
            let item = std::mem::replace(prev, sep_item.item);
            Some((item, sep_item.sep))
        })
    }

    /// The first `T` item.
    pub fn first(&self) -> &T {
        &self.head
    }

    /// The last `T` item.
    pub fn last(&self) -> &T {
        match self.rest.last() {
            Some(sep_item) => &sep_item.item,
            None => &self.head,
        }
    }
}

/// Sequence of three grammars, [`Deref`]-ing to the middle one.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Wrap<L, T, R> {
    /// The leading element.
    pub before: L,
    /// The wrapped value.
    pub wrapped: T,
    /// The trailing element.
    pub after: R,
}

impl<L, T, R> Wrap<L, T, R> {
    /// Construct a `Wrap` from its three parts.
    pub fn new(before: L, wrapped: T, after: R) -> Self {
        Self {
            before,
            wrapped,
            after,
        }
    }

    /// Apply `f` to the wrapped value, keeping `before`/`after` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Wrap<L, U, R> {
        Wrap {
            before: self.before,
            wrapped: f(self.wrapped),
            after: self.after,
        }
    }
}

impl<L, T, E, R> Wrap<L, Result<T, E>, R> {
    /// Swap `Wrap<L, Result<T, E>, R>` for `Result<Wrap<L, T, R>, E>`.
    pub fn transpose(self) -> Result<Wrap<L, T, R>, E> {
        Ok(Wrap {
            before: self.before,
            wrapped: self.wrapped?,
            after: self.after,
        })
    }
}

/// Sequence of two grammars, [`Deref`]-ing to the second.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Prefix<P, T> {
    prefix: P,
    /// The prefixed value.
    pub prefixed: T,
}

/// Sequence of two grammars, [`Deref`]-ing to the first.
#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(inline)]
pub struct Suffix<T, S> {
    suffixed: T,
    suffix: S,
}

impl<L, T, R> Deref for Wrap<L, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.wrapped
    }
}

impl<T, S> Deref for Suffix<T, S> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.suffixed
    }
}

impl<P, T> Deref for Prefix<P, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.prefixed
    }
}

impl<P, T> Prefix<P, T> {
    /// Apply `f` to the prefixed value, keeping `prefix` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prefix<P, U> {
        Prefix {
            prefix: self.prefix,
            prefixed: f(self.prefixed),
        }
    }
}

impl<P, T, E> Prefix<P, Result<T, E>> {
    /// Swap `Prefix<P, Result<T, E>>` for `Result<Prefix<P, T>, E>`.
    pub fn transpose(self) -> Result<Prefix<P, T>, E> {
        Ok(Prefix {
            prefix: self.prefix,
            prefixed: self.prefixed?,
        })
    }
}

impl<T, S> Suffix<T, S> {
    /// Apply `f` to the suffixed value, keeping `suffix` unchanged.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Suffix<U, S> {
        Suffix {
            suffixed: f(self.suffixed),
            suffix: self.suffix,
        }
    }
}

impl<T, S, E> Suffix<Result<T, E>, S> {
    /// Swap `Suffix<Result<T, E>, S>` for `Result<Suffix<T, S>, E>`.
    pub fn transpose(self) -> Result<Suffix<T, S>, E> {
        Ok(Suffix {
            suffixed: self.suffixed?,
            suffix: self.suffix,
        })
    }
}

/// Unwrap a grammar wrapper (e.g. [`Wrap`], [`Prefix`], [`Suffix`]) to get
/// at the value it wraps.
pub trait IntoInner<T> {
    /// Consume `self`, discarding whatever structure wrapped `T`.
    fn into_inner(self) -> T;
}

impl<L, T, R> IntoInner<T> for Wrap<L, T, R> {
    fn into_inner(self) -> T {
        self.wrapped
    }
}

impl<T, S> IntoInner<T> for Suffix<T, S> {
    fn into_inner(self) -> T {
        self.suffixed
    }
}

impl<P, T> IntoInner<T> for Prefix<P, T> {
    fn into_inner(self) -> T {
        self.prefixed
    }
}
