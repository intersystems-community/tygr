//! An enum's `parse_at`: its variants in order, from the first that can
//! start with the next byte.

use proc_macro2::TokenStream;
use quote::quote;

/// The variant count from which an enum looks up where to start.
const THRESHOLD: usize = 4;

/// The `parse_at` body: `arms`, one per variant, tried in order. With enough
/// variants, a table on the next byte says which is the first that can start
/// there, except at `Max`, where every variant is tried, so that each says
/// what it expected. The arms share `memo` slots (see `parse::memo_slots`),
/// `memo.0`, `memo.1`…, for the fields they start with in common.
pub(crate) fn dispatch(arms: &[TokenStream], firsts: &[TokenStream], memo: usize) -> TokenStream {
    let untried = (0..memo).map(|_| quote! { ::tygr::__private::Memo::Untried });
    let memo = (memo > 0).then(|| quote! { let mut memo = (#(#untried,)*); });
    let n = arms.len();
    let indices: Vec<usize> = (0..n).collect();
    if n < THRESHOLD {
        return quote! {
            #memo
            #( if let ::core::option::Option::Some(result) = (|| { #arms })() { return ::core::option::Option::Some(result); } )*
            ::core::option::Option::None
        };
    }
    quote! {
        #memo
        let start = if <__Level as ::tygr::Level>::IS_MAX {
            0
        } else {
            // The first variant that can start with each next byte (slot 256:
            // end of input), or `n` if none can.
            let first: &'static [usize; 257] = &const {
                let mut first = [#n; 257];
                let mut slot = 0;
                while slot < 257 {
                    let next = if slot < 256 { ::core::option::Option::Some(slot as u8) } else { ::core::option::Option::None };
                    first[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                        #indices
                    } else )* { #n };
                    slot += 1;
                }
                first
            };
            first[input.as_bytes().get(pos).map_or(256, |&byte| byte as usize)]
        };
        #( if start <= #indices {
            if let ::core::option::Option::Some(result) = (|| { #arms })() {
                return ::core::option::Option::Some(result);
            }
        } )*
        // Nothing matched: the variants skipped would have failed here.
        if !<__Level as ::tygr::Level>::IS_MAX {
            state.expect(pos, || ::core::unreachable!("`Max` doesn't dispatch"));
        }
        ::core::option::Option::None
    }
}
