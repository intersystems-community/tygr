//! An enum's `parse_at`: its variants in order, or, with enough of them, a
//! jump table on the next byte.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The variant count from which an enum dispatches on a table.
const THRESHOLD: usize = 4;

/// The `parse_at` body trying `arms` (one per variant) in order, and the
/// inherent fns it needs. With enough variants, a table on the next byte
/// skips to the first variant that can start there; when tracing, the
/// variants it skipped are reported with `fail_ats`, unless one matched
/// input.
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
        .chain([quote! {{
            if ::tygr::__private::TRACING {
                Self::__fail_at_skipped(input, pos, state);
            }
            ::core::option::Option::None
        }}]);
    let (indices, skips) = (0..n, 0..n);
    let items = quote! {
        #(
            #[inline]
            fn #cases<__Mode: ::tygr::Mode>(input: &str, pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> #output {
                if let ::core::option::Option::Some(result) = (|| { #arms })() {
                    // Matching empty, it rules out none of the variants skipped.
                    if ::tygr::__private::TRACING && ::tygr::first::contains::<#firsts>(::core::option::Option::None) && result.1 == pos {
                        Self::__fail_at_skipped(input, pos, state);
                    }
                    return ::core::option::Option::Some(result);
                }
                #nexts
            }
        )*

        // Report the variants the table skipped for the next byte: those
        // before the first that can start with it (all, if none can).
        #[cold]
        fn __fail_at_skipped(input: &str, #[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) {
            let skipped: &'static [usize; 257] = &const {
                let mut skipped = [#n; 257];
                let mut slot = 0;
                while slot < 257 {
                    let next = if slot < 256 { ::core::option::Option::Some(slot as u8) } else { ::core::option::Option::None };
                    skipped[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                        #skips
                    } else )* { #n };
                    slot += 1;
                }
                skipped
            };
            let k = skipped[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)];
            #( if #indices < k { let _ = #fail_ats; } )*
        }

        // No variant can start here: report them all.
        #[inline]
        fn __parse_miss<__Mode: ::tygr::Mode>(_input: &str, pos: usize, state: ::tygr::State) -> #output {
            if ::tygr::__private::TRACING {
                Self::__fail_at_variants(pos, state);
            }
            ::core::option::Option::None
        }
    };
    let fn_ty = quote! { fn(&str, usize, ::tygr::State<'_>) -> #output };
    let indices = 0..n;
    let body = quote! {
        // Slots 0-255: the next byte; 256: end of input. One per mode.
        let table: &'static [#fn_ty; 257] = &const {
            let cases: [#fn_ty; #n] = [#(Self::#cases::<__Mode>),*];
            let mut table: [#fn_ty; 257] = [Self::__parse_miss::<__Mode>; 257];
            let mut slot = 0;
            while slot < 257 {
                let next = if slot < 256 { ::core::option::Option::Some(slot as u8) } else { ::core::option::Option::None };
                table[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                    cases[#indices]
                } else )* { Self::__parse_miss::<__Mode> };
                slot += 1;
            }
            table
        };
        table[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)](input, pos, state)
    };
    (body, items)
}
