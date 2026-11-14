//! An enum's `parse_at`: its variants in order, or, with enough of them, a
//! dispatch table on the next byte.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The variant count from which an enum dispatches on a table.
const THRESHOLD: usize = 4;

/// The `parse_at` body trying `arms` (one per variant) in order, and the
/// inherent items it needs. With enough variants, a table on the next byte
/// skips to the first variant that can start there, except when recording:
/// then every variant is tried, so that each records what it expected.
pub(crate) fn dispatch(arms: &[TokenStream], firsts: &[TokenStream]) -> (TokenStream, TokenStream) {
    let n = arms.len();
    let in_order = quote! {
        #( if let ::core::option::Option::Some(result) = (|| { #arms })() { return ::core::option::Option::Some(result); } )*
        ::core::option::Option::None
    };
    if n < THRESHOLD {
        return (in_order, quote! {});
    }
    let output =
        quote! { ::core::option::Option<(<__Success as ::tygr::Success>::Output<Self>, usize)> };
    let cases: Vec<Ident> = (0..n).map(|k| format_ident!("__parse_case_{k}")).collect();
    let nexts = cases
        .iter()
        .skip(1)
        .map(|next| quote! { Self::#next::<__Success, __Level>(input, pos, state) })
        .chain([quote! { Self::__parse_miss::<__Success, __Level>(input, pos, state) }]);
    let indices: Vec<usize> = (0..n).collect();
    let items = quote! {
        #(
            #[inline]
            fn #cases<__Success: ::tygr::Success, __Level: ::tygr::Level>(input: &str, pos: usize, mut state: ::tygr::State<__Level>) -> #output {
                if let ::core::option::Option::Some(result) = (|| { #arms })() {
                    return ::core::option::Option::Some(result);
                }
                #nexts
            }
        )*

        // The first variant that can start with each next byte (slot 256: end
        // of input), or `n` if none can.
        const __FIRST_CASE: [usize; 257] = {
            let mut first_case = [#n; 257];
            let mut slot = 0;
            while slot < 257 {
                let next = if slot < 256 { ::core::option::Option::Some(slot as u8) } else { ::core::option::Option::None };
                first_case[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                    #indices
                } else )* { #n };
                slot += 1;
            }
            first_case
        };

        // Nothing matched: the variants skipped would have failed here.
        #[inline]
        fn __parse_miss<__Success: ::tygr::Success, __Level: ::tygr::Level>(_input: &str, pos: usize, mut state: ::tygr::State<__Level>) -> #output {
            ::tygr::__private::miss(&mut state, pos);
            ::core::option::Option::None
        }
    };
    let fn_ty = quote! { fn(&str, usize, ::tygr::State<'_, __Level>) -> #output };
    let body = quote! {
        // Recording, every variant is tried, so that each records what it
        // expected; otherwise the table skips those that can't start here.
        if <__Level as ::tygr::Level>::RECORD {
            #in_order
        } else {
            // One table per mode and level.
            let table: &'static [#fn_ty; 257] = &const {
                let cases: [#fn_ty; #n + 1] = [#(Self::#cases::<__Success, __Level>,)* Self::__parse_miss::<__Success, __Level>];
                let mut table: [#fn_ty; 257] = [Self::__parse_miss::<__Success, __Level>; 257];
                let mut slot = 0;
                while slot < 257 {
                    table[slot] = cases[Self::__FIRST_CASE[slot]];
                    slot += 1;
                }
                table
            };
            table[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)](input, pos, state)
        }
    };
    (body, items)
}
