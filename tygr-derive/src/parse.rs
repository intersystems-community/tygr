//! A derived `Parse`.

use crate::dispatch::dispatch;
use crate::model::{Conversion, Field, Model, Shape, Variant};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
use std::collections::HashMap;
use syn::Index;

/// The `Parse` impl.
pub(crate) fn expand(model: &Model) -> TokenStream {
    let Model {
        ident,
        name,
        hidden,
        inline,
        validated,
        shape,
        ..
    } = model;
    // Every parameter needs `'static`, for `TypeId`.
    let mut generics = model.bounded(quote! { ::tygr::Parse });
    for param in generics.type_params_mut() {
        param.bounds.push(syn::parse_quote!('static));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (first, parse_at, def) = match shape {
        Shape::Struct(variant) | Shape::Converted(variant) => (
            first(variant),
            parse(model, variant, &[]),
            sequence(variant),
        ),
        Shape::Enum(variants) => {
            let firsts: Vec<_> = variants.iter().map(first).collect();
            let (slots, memo) = memo_slots(variants);
            let arms: Vec<_> = variants
                .iter()
                .zip(&slots)
                .map(|(variant, slots)| parse(model, variant, slots))
                .collect();
            let sequences = variants.iter().map(sequence);
            (
                firsts.iter().fold(
                    quote! { ::tygr::first::Required<::tygr::first::byte_set::Empty> },
                    |first, variant| quote! { <#first as ::tygr::first::First>::Alt<#variant> },
                ),
                dispatch(&arms, &firsts, memo),
                quote! { ::tygr::bnf::Expr::alternation(::std::vec![#(#sequences),*]) },
            )
        }
    };
    let def = if *validated {
        quote! { ::tygr::bnf::Expr::side_condition(#def, <Self as ::tygr::Validate>::REQUIREMENT) }
    } else {
        def
    };
    let sources = model.fields().map(Field::source);
    let visit_parts = quote! { #(<#sources as ::tygr::Parse>::visit_rules(rules);)* };
    // A hidden grammar refers to nothing; an inline one has no rule of its
    // own, unless it refers to itself.
    let (to_bnf, visit_rules) = if *hidden {
        (quote! { ::tygr::bnf::Expr::Hidden }, quote! {})
    } else if *inline {
        (
            quote! { ::tygr::__private::inline_bnf::<Self>(#name, || #def) },
            quote! {
                ::tygr::__private::visit_inline::<Self>(rules, #name, || #def, |rules| {
                    #visit_parts
                });
            },
        )
    } else {
        (
            quote! { ::tygr::bnf::Expr::RuleRef(::std::string::ToString::to_string(#name)) },
            quote! {
                if rules.insert::<Self>(#name, || #def) {
                    #visit_parts
                }
            },
        )
    };
    let parse_at = with_node(model, parse_at);
    // `__Success` and `__Level`, not `M` and `L`, so they can't shadow a type
    // parameter of the user's (short of one so named, names the derive
    // reserves).
    quote! {
        impl #impl_generics ::tygr::Parse for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline(always)]
            fn parse_at<__Success: ::tygr::Success, __Level: ::tygr::Level>(input: &str, pos: usize, mut state: ::tygr::State<__Level>) -> ::core::option::Option<(<__Success as ::tygr::Success>::Output<Self>, usize)> {
                #parse_at
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #to_bnf
            }

            fn visit_rules(rules: &mut ::tygr::bnf::Rules) {
                #visit_rules
            }
        }
    }
}

/// `body` in this rule's node, unless it's inline.
fn with_node(model: &Model, body: TokenStream) -> TokenStream {
    if model.inline {
        return body;
    }
    let name = &model.name;
    quote! {
        let mut state = state.node(#name, pos);
        #body
    }
}

/// The FIRST set of `variant`'s fields in sequence.
fn first(variant: &Variant) -> TokenStream {
    variant.fields.iter().map(Field::source).fold(
        quote! { ::tygr::first::Optional<::tygr::first::byte_set::Empty> },
        |first, source| quote_spanned! { variant.span => <#first as ::tygr::first::First>::Seq<#source> },
    )
}

/// For each variant, the memo slot of each field that another variant parses
/// the same way after the same fields, and how many slots there are: one per
/// node of the variants' field trie that two or more pass through, where a
/// later variant finds what an earlier one parsed, or that it failed, at the
/// one position they share.
fn memo_slots(variants: &[Variant]) -> (Vec<Vec<Option<usize>>>, usize) {
    let paths: Vec<Vec<String>> = variants
        .iter()
        .map(|variant| {
            let mut prefix = String::new();
            variant
                .fields
                .iter()
                .map(|field| {
                    prefix += &format!("{}\n", parse_call(field, &TokenStream::new()).1);
                    prefix.clone()
                })
                .collect()
        })
        .collect();
    let mut users = HashMap::new();
    for prefix in paths.iter().flatten() {
        *users.entry(prefix).or_insert(0) += 1;
    }
    let mut slots = HashMap::new();
    let per_field = paths
        .iter()
        .map(|path| {
            path.iter()
                .map(|prefix| {
                    let n = slots.len();
                    (users[prefix] >= 2).then(|| *slots.entry(prefix).or_insert(n))
                })
                .collect()
        })
        .collect();
    (per_field, slots.len())
}

/// Parse each field, then build the value; or, if `validated`, build it
/// whatever the mode and check it. A field with a memo slot (`slots`, by
/// field) is parsed once for every variant sharing it, and its value waits in
/// the slot for the variant that succeeds.
fn parse(model: &Model, variant: &Variant, slots: &[Option<usize>]) -> TokenStream {
    let mode = if model.validated {
        quote! { ::tygr::success::Value }
    } else {
        quote! { __Success }
    };
    let name = &model.name;
    let slot = |i: usize| slots.get(i).copied().flatten();
    let steps = variant
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| parse_field(field, &mode, slot(i)));
    let takes = variant.fields.iter().enumerate().filter_map(|(i, field)| {
        let (binding, slot) = (field.binding()?, Index::from(slot(i)?));
        Some(quote! { let #binding = memo.#slot.take(); })
    });
    let constructor = &variant.constructor;
    let bindings: Vec<_> = variant.fields.iter().filter_map(Field::binding).collect();
    if model.validated {
        return quote! {
            let start = pos;
            #(#steps)*
            #(#takes)*
            let value = #constructor;
            if !::tygr::Validate::validate(&value) {
                state.reject(#name, start..pos, <Self as ::tygr::Validate>::REQUIREMENT);
                return ::core::option::Option::None;
            }
            ::core::option::Option::Some((<__Success as ::tygr::Success>::output(|| value), pos))
        };
    }
    // The fields' outputs combined, each `and_then` reaching its value only
    // when building.
    let output = bindings.iter().rev().fold(
        quote! { <__Success as ::tygr::Success>::output(|| #constructor) },
        |output, binding| quote! { <__Success as ::tygr::Success>::and_then(#binding, |#binding| #output) },
    );
    quote! {
        #(#steps)*
        #(#takes)*
        ::core::option::Option::Some((#output, pos))
    }
}

/// Parse `field` in `mode`, binding it; or, in memo slot `slot`, reach the
/// position an earlier variant parsed it to, or fail as it did, parsing only
/// if none did (its value then waits in the slot).
fn parse_field(field: &Field, mode: &TokenStream, slot: Option<usize>) -> TokenStream {
    let (binding, parse) = parse_call(field, mode);
    match slot.map(Index::from) {
        Some(slot) => quote! { let pos = memo.#slot.end(|| #parse)?; },
        None => quote! { let (#binding, pos) = #parse?; },
    }
}

/// `field`'s binding, and the call parsing it in `mode`: as its type, or
/// converted; a dropped one is converted to `()`.
fn parse_call(field: &Field, mode: &TokenStream) -> (TokenStream, TokenStream) {
    let (binding, ty, kind) = match field {
        Field::Kept {
            binding,
            ty,
            conversion: None,
            ..
        } => {
            return (
                quote! { #binding },
                quote! { <#ty as ::tygr::Parse>::parse_at::<#mode, __Level>(input, pos, state.reborrow()) },
            );
        }
        Field::Kept {
            binding,
            ty,
            conversion: Some(Conversion { kind, .. }),
            ..
        } => (quote! { #binding }, quote! { #ty }, kind),
        Field::Dropped { kind, .. } => (quote! { _ }, quote! { () }, kind),
    };
    let (source, helper, target) = (field.source(), kind.helper(), field.target());
    (
        binding,
        quote! { ::tygr::__private::#helper::<#source, #ty, #mode, __Level>(input, pos, state.reborrow(), #target) },
    )
}

/// `variant`'s fields as an EBNF sequence; a fallible conversion is footnoted.
fn sequence(variant: &Variant) -> TokenStream {
    let bnfs = variant.fields.iter().map(|field| {
        let source = field.source();
        let bnf = quote! { <#source as ::tygr::Parse>::to_bnf() };
        match field.requirement() {
            Some(requirement) => quote! { ::tygr::bnf::Expr::side_condition(#bnf, #requirement) },
            None => bnf,
        }
    });
    quote! { ::tygr::bnf::Expr::sequence(::std::vec![#(#bnfs),*]) }
}

/// Reads every field, as a derived `Print` would: a grammar's fields are
/// its syntax, so the dead-code lint shouldn't flag them as never read.
/// Nothing calls it: the `allow(dead_code)` is what makes rustc walk its
/// body, since allowed items seed the dead-code analysis. A converted
/// type's fields are its own data, read or not.
pub(crate) fn read_fields(model: &Model) -> TokenStream {
    if let Shape::Converted(_) = model.shape {
        return quote! {};
    }
    let ident = &model.ident;
    let (impl_generics, ty_generics, where_clause) = model.generics.split_for_impl();
    let constructors = model.variants().iter().map(|variant| &variant.constructor);
    let value = model.matched(quote! { value });
    quote! {
        const _: () = {
            #[allow(dead_code)]
            fn read_fields #impl_generics (value: &#ident #ty_generics) #where_clause {
                match #value {
                    #(#constructors => {})*
                }
            }
        };
    }
}
