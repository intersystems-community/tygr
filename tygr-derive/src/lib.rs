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

/// Derive `Parse` and `Print` together, reading the `#[tygr(...)]`
/// attributes of both.
#[proc_macro_derive(Grammar, attributes(tygr))]
pub fn derive_grammar(input: TokenStream) -> TokenStream {
    derive(input, |model| {
        let (parse, print) = (parse::expand(model), print::expand(model));
        quote! { #parse #print }
    })
}

/// Derive `Parse` for a `struct`, a sequence of its fields, or an `enum`, an
/// alternation of its variants; or, with `#[tygr(try_from = S)]` or another
/// conversion on the type, parse `S` and convert it into the whole value.
/// Optional on the type: `#[tygr(name = "…", hidden, inline, validated,
/// unbounded(T))]`; on a field: `#[tygr(try_from = S)]`,
/// `try_from_infallible`, `from_str` or `from_str_infallible` (see `tygr`'s
/// crate docs).
#[proc_macro_derive(Parse, attributes(tygr))]
pub fn derive_parse(input: TokenStream) -> TokenStream {
    derive(input, |model| {
        let (parse, read_fields) = (parse::expand(model), parse::read_fields(model));
        quote! { #parse #read_fields }
    })
}

/// Derive `Print` for a `struct` or `enum`, printing its fields, or the
/// variant's, in turn. A field or type converted with `#[tygr(try_from = S)]`
/// or another conversion has no grammar to print with, so it needs
/// `#[tygr(print_to = f)]`, an `fn(&T, &mut String)`; `#[tygr(unbounded(T))]`
/// keeps only the bounds you wrote (see `tygr`'s crate docs).
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
