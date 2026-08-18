//! # tygr — TYpe into Grammar Routines
//!
//! Define grammars as Rust types and derive the routine work of grammar
//! handling: a **parser**, a **printer**, and a **presentation** (in EBNF).
//!
//! ## Design
//!
//! | Rust construct | EBNF (ISO 14977)             |
//! |----------------|------------------------------|
//! | `struct`       | concatenation (`A , B , C`)  |
//! | `enum`         | alternation (`A \| B`)       |
//! | `Vec<T>`       | repetition (`{ T }`)         |
//! | `Option<T>`    | optional (`[ T ]`)           |
//! | `Box<T>`       | recursive indirection        |
//! | `(A, B, …)`    | inline concatenation         |
//! | `Hidden<T>`    | parsed/printed, hidden in BNF rules|
//!
//! Rule references are just type references — if a `rule_ref` is wrong, the
//! program simply won't compile.
//!
//! ## Quick example
//!
//! ```rust
//! use tygr::*;
//!
//! // -- character class --
//! char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());
//!
//! // -- AST types --
//!
//! #[derive(Grammar)]
//! pub struct Number(pub StringOf1<IsDigit>);
//!
//! #[derive(Grammar)]
//! pub enum Factor {
//!     Paren(StringEq!("("), Box<Expr>, StringEq!(")")),
//!     Num(Number),
//! }
//!
//! #[derive(Grammar)]
//! pub struct Expr(pub Factor, pub Vec<(StringEq!("+"), Factor)>);
//!
//! let e = Expr::parse("1+2+(3)").unwrap();
//! assert_eq!(e.print(), "1+2+(3)");
//! ```
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

/// Derive `Grammar` (and `GrammarRule`) for a struct or enum.
///
/// The `#[grammar(...)]` attribute is optional. Name defaults to the type
/// name. Use `#[grammar(hidden)]` to hide from EBNF output.
///
/// ```
/// # use tygr::*;
/// # char_class!(IsDigit, "digit", |ch| ch.is_ascii_digit());
/// # char_class!(IsSpace, "space", |ch| matches!(ch, ' ' | '\t'));
/// # #[derive(Grammar)]
/// # struct Term(StringOf1<IsDigit>);
/// # #[derive(Grammar)]
/// # struct AddOp(StringEq!("+"));
/// #[derive(Grammar)]
/// struct Expr(Term, Vec<(AddOp, Term)>);
///
/// #[derive(Grammar)]
/// #[grammar(hidden)]
/// struct Ws(StringOf<IsSpace>);
/// ```
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

/// Expand a string literal into a **case-insensitive** literal-token type.
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

pub trait FilterResult {
    fn be_valid(self) -> Option<&'static str>;
}

impl FilterResult for bool {
    fn be_valid(self) -> Option<&'static str> {
        if self { None } else { Some("be valid") }
    }
}

impl FilterResult for Option<&'static str> {
    fn be_valid(self) -> Option<&'static str> {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(transparent)]
#[doc(hidden)]
pub struct SepItem<S, T> {
    sep: S,
    item: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(transparent)]
pub struct VecSep<T, S> {
    head: Box<T>,
    rest: Vec<SepItem<S, T>>,
}

impl<T, S> VecSep<T, S> {
    pub fn items(&self) -> impl Iterator<Item = &T> {
        vec![self.head.as_ref()]
            .into_iter()
            .chain(self.rest.iter().map(|sep_item| &sep_item.item))
    }

    pub fn seps(&self) -> impl Iterator<Item = &S> {
        self.rest.iter().map(|sep_item| &sep_item.sep)
    }

    pub fn into_item_sep(self) -> impl Iterator<Item = (T, S)> {
        self.rest.into_iter().scan(*self.head, |prev, sep_item| {
            let item = std::mem::replace(prev, sep_item.item);
            Some((item, sep_item.sep))
        })
    }

    pub fn first(&self) -> &T {
        &self.head
    }

    pub fn last(&self) -> &T {
        match self.rest.last() {
            Some(sep_item) => &sep_item.item,
            None => &self.head,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(transparent)]
pub struct Wrap<L, T, R> {
    pub before: L,
    pub wrapped: T,
    pub after: R,
}

impl<L, T, R> Wrap<L, T, R> {
    pub fn new(before: L, wrapped: T, after: R) -> Self {
        Self {
            before,
            wrapped,
            after,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Wrap<L, U, R> {
        Wrap {
            before: self.before,
            wrapped: f(self.wrapped),
            after: self.after,
        }
    }
}

impl<L, T, E, R> Wrap<L, Result<T, E>, R> {
    pub fn transpose(self) -> Result<Wrap<L, T, R>, E> {
        Ok(Wrap {
            before: self.before,
            wrapped: self.wrapped?,
            after: self.after,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(transparent)]
pub struct Prefix<P, T> {
    prefix: P,
    pub prefixed: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Grammar)]
#[grammar(transparent)]
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
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prefix<P, U> {
        Prefix {
            prefix: self.prefix,
            prefixed: f(self.prefixed),
        }
    }
}

impl<P, T, E> Prefix<P, Result<T, E>> {
    pub fn transpose(self) -> Result<Prefix<P, T>, E> {
        Ok(Prefix {
            prefix: self.prefix,
            prefixed: self.prefixed?,
        })
    }
}

impl<T, S> Suffix<T, S> {
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Suffix<U, S> {
        Suffix {
            suffixed: f(self.suffixed),
            suffix: self.suffix,
        }
    }
}

impl<T, S, E> Suffix<Result<T, E>, S> {
    pub fn transpose(self) -> Result<Suffix<T, S>, E> {
        Ok(Suffix {
            suffixed: self.suffixed?,
            suffix: self.suffix,
        })
    }
}

pub trait IntoInner<T> {
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
