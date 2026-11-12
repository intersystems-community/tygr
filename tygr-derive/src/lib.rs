//! The derive and literal macros that `tygr` re-exports.

mod dispatch;
mod literal;
mod model;
mod parse;
mod print;

use model::Model;
use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// Derive `Parse` and `Print` for a `struct` or `enum`: a `struct` is a
/// sequence of its fields, an `enum` an alternation of its variants.
///
/// `#[tygr(name = "…", hidden, inline, validated)]` on the type, and
/// `#[tygr(try_from/try_from_infallible/from_str/from_str_infallible = S,
/// print_to = f)]` on a field, are optional, except that a converted field
/// prints only with `print_to` (see `tygr`'s crate docs).
#[proc_macro_derive(Grammar, attributes(tygr))]
pub fn derive_grammar(input: TokenStream) -> TokenStream {
    derive(input, |model| {
        let (parse, print) = (parse::expand(model), print::expand(model));
        quote! { #parse #print }
    })
}

/// Derive `Parse` alone, as `Grammar` derives it.
#[proc_macro_derive(Parse, attributes(tygr))]
pub fn derive_parse(input: TokenStream) -> TokenStream {
    derive(input, |model| {
        let (parse, read_fields) = (parse::expand(model), parse::read_fields(model));
        quote! { #parse #read_fields }
    })
}

/// Derive `Print` alone, as `Grammar` derives it.
#[proc_macro_derive(Print, attributes(tygr))]
pub fn derive_print(input: TokenStream) -> TokenStream {
    derive(input, print::expand)
}

/// Read `input` into a model, then generate with `expand`.
fn derive(input: TokenStream, expand: impl FnOnce(&Model) -> TokenStream2) -> TokenStream {
    let input = syn::parse_macro_input!(input as syn::DeriveInput);
    Model::new(&input)
        .map(|model| expand(&model))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// `Token<G>`, `G` holding each char's UTF-8 bytes as `ByteEq`s. Not doc
// comments: `tygr`'s re-exports have their own, and rustdoc would show both.
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
