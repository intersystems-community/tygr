//! An enum's `parse_at`: its variants in order, or, with enough of them, a
//! jump table on the next byte.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Ident;

/// The variant count from which an enum dispatches on a table.
const THRESHOLD: usize = 4;

/// The `parse_at` body trying `arms` (one per variant) in order, and the
/// inherent fns it needs. With enough variants, a table on the next byte
/// skips to the first variant that can start there.
pub(crate) fn dispatch(arms: &[TokenStream], firsts: &[TokenStream]) -> (TokenStream, TokenStream) {
    let n = arms.len();
    if n < THRESHOLD {
        let body = quote! {
            #( if let Some(result) = (|| { #arms })() { return Some(result); } )*
            None
        };
        return (body, quote! {});
    }
    let output = quote! { Option<(<__Mode as ::tygr::Mode>::Output<Self>, usize)> };
    let cases: Vec<Ident> = (0..n).map(|k| format_ident!("parse_case_{k}")).collect();
    let nexts = cases
        .iter()
        .skip(1)
        .map(|next| quote! { Self::#next::<__Mode>(input, pos, state) })
        .chain([quote! { None }]);
    let items = quote! {
        #(
            #[inline]
            fn #cases<__Mode: ::tygr::Mode>(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> #output {
                if let Some(result) = (|| { #arms })() { return Some(result); }
                #nexts
            }
        )*

        // No variant can start here: report them all.
        #[inline]
        fn parse_miss<__Mode: ::tygr::Mode>(_input: &[u8], pos: usize, state: ::tygr::State) -> #output {
            if ::tygr::__private::TRACING {
                Self::fail_at_variants(pos, state);
            }
            None
        }
    };
    let fn_ty = quote! { fn(&[u8], usize, ::tygr::State<'_>) -> #output };
    let indices = 0..n;
    let body = quote! {
        // Slots 0-255: the next byte; 256: end of input. One per mode.
        let table: &'static [#fn_ty; 257] = &const {
            let cases: [#fn_ty; #n] = [#(Self::#cases::<__Mode>),*];
            let mut table: [#fn_ty; 257] = [Self::parse_miss::<__Mode>; 257];
            let mut slot = 0;
            while slot < 257 {
                let next = if slot < 256 { Some(slot as u8) } else { None };
                table[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                    cases[#indices]
                } else )* { Self::parse_miss::<__Mode> };
                slot += 1;
            }
            table
        };
        table[input.get(pos).map_or(256, |&byte| byte as usize)](input, pos, state)
    };
    (body, items)
}
