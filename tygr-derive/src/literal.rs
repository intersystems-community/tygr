//! `StringEq!` and `StringEqCI!`: a literal as nested byte grammars.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;

/// `Token<G>`, `G` nesting each char's `char_grammar` as pairs.
pub(crate) fn expand(
    input: TokenStream,
    macro_name: &str,
    char_grammar: fn(char) -> TokenStream2,
) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    if value.is_empty() {
        let message = format!("{macro_name}!() requires a non-empty literal");
        return syn::Error::new(lit.span(), message)
            .to_compile_error()
            .into();
    }
    let tuple = right_nested(value.chars().map(char_grammar), pair);
    quote! { ::tygr::__private::Token<#tuple> }.into()
}

/// `ch`'s UTF-8 bytes as a tuple of `ByteEq`s.
pub(crate) fn char_eq(ch: char) -> TokenStream2 {
    byte_tuple(ch.to_string().as_bytes())
}

/// An `Either` over `ch`'s case variants, or its own tuple if it has none.
pub(crate) fn char_eq_ci(ch: char) -> TokenStream2 {
    let variants = case_variants(ch)
        .into_iter()
        .map(|variant| byte_tuple(variant.as_bytes()));
    right_nested(
        variants,
        |first, rest| quote! { ::tygr::__private::Either<#first, #rest> },
    )
}

/// `ch`, then its Unicode case mappings longest first, so ordered choice
/// never stops at a shorter prefix (no char prefixes its own mappings).
fn case_variants(ch: char) -> Vec<String> {
    let me = ch.to_string();
    let mut others: Vec<String> = vec![ch.to_lowercase().collect(), ch.to_uppercase().collect()];
    others.retain(|other| *other != me);
    others.sort_by_key(|other| std::cmp::Reverse(other.len()));
    others.dedup();
    let mut variants = vec![me];
    variants.extend(others);
    variants
}

/// `ByteEq`s for non-empty `bytes`, nested as pairs.
fn byte_tuple(bytes: &[u8]) -> TokenStream2 {
    right_nested(
        bytes
            .iter()
            .map(|byte| quote! { ::tygr::__private::ByteEq<#byte> }),
        pair,
    )
}

/// `(first, rest)`.
fn pair(first: TokenStream2, rest: TokenStream2) -> TokenStream2 {
    quote! { (#first, #rest) }
}

/// `[a, b, c]` → `join(a, join(b, c))`.
fn right_nested(
    items: impl DoubleEndedIterator<Item = TokenStream2>,
    join: impl Fn(TokenStream2, TokenStream2) -> TokenStream2,
) -> TokenStream2 {
    let mut items = items.rev();
    let last = items.next().expect("non-empty");
    items.fold(last, |rest, item| join(item, rest))
}
