//! `StringEq!` and `StringEqCI!`: a literal as nested byte atoms.

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
    let tuple = balanced(&value.chars().map(char_grammar).collect::<Vec<_>>(), &pair);
    quote! { ::tygr::__private::Token<#tuple> }.into()
}

/// `ch`'s UTF-8 bytes as a tuple of `ByteEq`s.
pub(crate) fn char_eq(ch: char) -> TokenStream2 {
    byte_tuple(ch.to_string().as_bytes())
}

/// An `Either` over `ch`'s case variants, or its own tuple if it has none.
pub(crate) fn char_eq_ci(ch: char) -> TokenStream2 {
    let variants: Vec<TokenStream2> = case_variants(ch)
        .iter()
        .map(|variant| byte_tuple(variant.as_bytes()))
        .collect();
    balanced(&variants, &|first, rest| {
        quote! { ::tygr::__private::Either<#first, #rest> }
    })
}

/// `ch`, then its Unicode case mappings and theirs, longest first, so
/// ordered choice never stops at a shorter prefix (no variant prefixes
/// another).
fn case_variants(ch: char) -> Vec<String> {
    let mut variants = vec![ch.to_string()];
    let mut i = 0;
    while let Some(variant) = variants.get(i).cloned() {
        for mapped in [variant.to_lowercase(), variant.to_uppercase()] {
            if !variants.contains(&mapped) {
                variants.push(mapped);
            }
        }
        i += 1;
    }
    variants[1..].sort_by_key(|other| std::cmp::Reverse(other.len()));
    variants
}

/// `ByteEq`s for non-empty `bytes`, nested as pairs.
fn byte_tuple(bytes: &[u8]) -> TokenStream2 {
    let atoms: Vec<TokenStream2> = bytes
        .iter()
        .map(|byte| quote! { ::tygr::__private::ByteEq<#byte> })
        .collect();
    balanced(&atoms, &pair)
}

/// `(first, rest)`.
fn pair(first: TokenStream2, rest: TokenStream2) -> TokenStream2 {
    quote! { (#first, #rest) }
}

/// `[a, b, c, d]` → `join(join(a, b), join(c, d))`: log-deep, so a long
/// literal's type stays within the compiler's recursion limit.
fn balanced(
    items: &[TokenStream2],
    join: &dyn Fn(TokenStream2, TokenStream2) -> TokenStream2,
) -> TokenStream2 {
    match items {
        [] => unreachable!("non-empty"),
        [item] => item.clone(),
        _ => {
            let (left, right) = items.split_at(items.len() / 2);
            join(balanced(left, join), balanced(right, join))
        }
    }
}
