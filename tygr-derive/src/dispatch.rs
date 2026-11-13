//! An enum's `parse_at`: its variants in order, or, with enough of them, a
//! dispatch table on the next byte.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The variant count from which an enum dispatches on a table.
const THRESHOLD: usize = 4;

/// The `parse_at` body trying `arms` (one per variant) in order, and the
/// inherent items it needs. With enough variants, a table on the next byte
/// skips to the first variant that can start there; the variants it skipped
/// are reported with `fail_ats`, unless one matched input.
pub(crate) fn dispatch(
    arms: &[TokenStream],
    firsts: &[TokenStream],
    fail_ats: &[TokenStream],
) -> (TokenStream, TokenStream) {
    let n = arms.len();
    if n < THRESHOLD {
        let body = quote! {
            #( if let ::core::option::Option::Some(result) = (|| { #arms })() { return ::core::option::Option::Some(result); } )*
            ::core::option::Option::None
        };
        return (body, quote! {});
    }
    let output = quote! { ::core::option::Option<(<__Mode as ::tygr::Mode>::Output<Self>, usize)> };
    let cases: Vec<Ident> = (0..n).map(|k| format_ident!("__parse_case_{k}")).collect();
    let nexts = cases
        .iter()
        .skip(1)
        .map(|next| quote! { Self::#next::<__Mode>(input, pos, state) })
        .chain([quote! { Self::__parse_miss::<__Mode>(input, pos, state) }]);
    let indices: Vec<usize> = (0..n).collect();
    let items = quote! {
        #(
            #[inline]
            fn #cases<__Mode: ::tygr::Mode>(input: &str, pos: usize, mut state: ::tygr::State<__Mode>) -> #output {
                if let ::core::option::Option::Some(result) = (|| { #arms })() {
                    // Matching empty, it rules out none of the variants skipped.
                    if const { ::tygr::first::contains::<#firsts>(::core::option::Option::None) } && result.1 == pos {
                        Self::__fail_at_skipped(input, pos, state);
                    }
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

        // Report the variants the table skipped: those before the first that
        // can start with the next byte.
        #[cold]
        fn __fail_at_skipped<__Mode: ::tygr::Mode>(input: &str, pos: usize, mut state: ::tygr::State<__Mode>) {
            let k = Self::__FIRST_CASE[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)];
            #( if #indices < k { let _ = #fail_ats; } )*
        }

        // Nothing matched: report the variants the table skipped.
        #[inline]
        fn __parse_miss<__Mode: ::tygr::Mode>(input: &str, pos: usize, state: ::tygr::State<__Mode>) -> #output {
            Self::__fail_at_skipped(input, pos, state);
            ::core::option::Option::None
        }
    };
    let fn_ty = quote! { fn(&str, usize, ::tygr::State<'_, __Mode>) -> #output };
    let body = quote! {
        // One table per mode.
        let table: &'static [#fn_ty; 257] = &const {
            let cases: [#fn_ty; #n + 1] = [#(Self::#cases::<__Mode>,)* Self::__parse_miss::<__Mode>];
            let mut table: [#fn_ty; 257] = [Self::__parse_miss::<__Mode>; 257];
            let mut slot = 0;
            while slot < 257 {
                table[slot] = cases[Self::__FIRST_CASE[slot]];
                slot += 1;
            }
            table
        };
        table[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)](input, pos, state)
    };
    (body, items)
}
