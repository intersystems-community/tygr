//! The derive and literal macros that `tygr` re-exports.

mod dispatch;
mod literal;
mod model;
mod parse;
mod print;

use model::Model;
use proc_macro::TokenStream;
use quote::quote;

/// Derive `Grammar` (and `GrammarRule`) for a `struct` or `enum`.
///
/// A `struct` is a sequence of its fields; an `enum` an alternation of its
/// variants. `#[tygr(name = "…", hidden, inline, validated)]` are
/// optional, as is converting a field with
/// `#[tygr(from/try_from/from_str/from_str_infallible = S)]` (see `tygr`'s
/// crate docs).
#[proc_macro_derive(Grammar, attributes(tygr))]
pub fn derive_grammar(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    Model::new(&input)
        .map(|model| parse::expand(&model, print::print_to(&model)))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// `Token<G>`, `G` holding each char's UTF-8 bytes as `ByteEq`s. Not doc
// comments: these would show on `tygr`'s re-exports.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEq(input: TokenStream) -> TokenStream {
    literal::expand(input, "StringEq", literal::char_eq)
}

// Like `StringEq!`, each char an `Either` over its case variants.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEqCI(input: TokenStream) -> TokenStream {
    literal::expand(input, "StringEqCI", literal::char_eq_ci)
}

/// `default_name` as a string literal, for `tygr::char_class!`.
#[doc(hidden)]
#[proc_macro]
pub fn __default_name(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as syn::Ident);
    let name = model::default_name(&ident);
    quote! { #name }.into()
}
