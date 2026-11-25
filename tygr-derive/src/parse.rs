//! A derived `Parse`.

use crate::dispatch::dispatch;
use crate::model::{Conversion, Field, Model, Shape, Variant};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};
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
    let (first, parse_at, def, inherent) = match shape {
        Shape::Struct(variant) | Shape::Converted(variant) => (
            first(variant),
            parse(model, variant, &vec![None; variant.fields.len()]),
            sequence(variant),
            None,
        ),
        Shape::Enum(variants) => {
            let firsts: Vec<_> = variants.iter().map(first).collect();
            let (slots, memo) = memo_slots(variants, &mode(model));
            let arms: Vec<_> = variants
                .iter()
                .zip(&slots)
                .map(|(variant, slots)| parse(model, variant, slots))
                .collect();
            let sequences = variants.iter().map(sequence);
            let (parse_at, items) =
                dispatch(&arms, &firsts, (!memo.is_empty()).then_some(&memo[..]));
            let inherent = (!items.is_empty()).then(
                || quote! { impl #impl_generics #ident #ty_generics #where_clause { #items } },
            );
            (
                firsts.iter().fold(
                    quote! { ::tygr::first::Required<::tygr::first::byte_set::Empty> },
                    |first, variant| quote! { <#first as ::tygr::first::First>::Alt<#variant> },
                ),
                parse_at,
                quote! { ::tygr::bnf::Expr::alternation(::std::vec![#(#sequences),*]) },
                inherent,
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
        #inherent

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

/// The success a variant's fields parse in: the value whatever the caller
/// asked for, if the type is validated.
fn mode(model: &Model) -> TokenStream {
    if model.validated {
        quote! { ::tygr::success::Value }
    } else {
        quote! { __Success }
    }
}

/// For each variant, the memo slot of each field that it and another
/// variant parse as the same field after the same fields, and the slots'
/// types. The slots are the nodes of the variants' field trie that two or
/// more variants pass through: a later variant finds there what an earlier
/// one parsed, or that it failed, at the one position they share.
fn memo_slots(
    variants: &[Variant],
    mode: &TokenStream,
) -> (Vec<Vec<Option<usize>>>, Vec<TokenStream>) {
    let mut nodes: Vec<(Vec<String>, usize, TokenStream)> = Vec::new();
    let paths: Vec<Vec<usize>> = variants
        .iter()
        .map(|variant| {
            let mut path = Vec::new();
            variant
                .fields
                .iter()
                .map(|field| {
                    path.push(key(field));
                    match nodes.iter().position(|(node, ..)| *node == path) {
                        Some(index) => {
                            nodes[index].1 += 1;
                            index
                        }
                        None => {
                            nodes.push((path.clone(), 1, output(field, mode)));
                            nodes.len() - 1
                        }
                    }
                })
                .collect()
        })
        .collect();
    let mut slots = vec![None; nodes.len()];
    let mut types = Vec::new();
    for (index, (_, users, ty)) in nodes.iter().enumerate() {
        if *users >= 2 {
            slots[index] = Some(types.len());
            types.push(ty.clone());
        }
    }
    let slots = paths
        .iter()
        .map(|path| path.iter().map(|&index| slots[index]).collect())
        .collect();
    (slots, types)
}

/// What makes two fields the same parse: how they're parsed, from what, into
/// what.
fn key(field: &Field) -> String {
    let (helper, ty) = match field {
        Field::Kept {
            ty,
            conversion: None,
            ..
        } => ("parse".to_string(), quote! { #ty }),
        Field::Kept {
            ty,
            conversion: Some(Conversion { kind, .. }),
            ..
        } => (kind.helper().to_string(), quote! { #ty }),
        Field::Dropped { kind, .. } => (kind.helper().to_string(), quote! { () }),
    };
    let source = field.source();
    format!("{helper} {} {ty}", quote! { #source })
}

/// The memo slot for what parsing `field` yields in `mode`.
fn output(field: &Field, mode: &TokenStream) -> TokenStream {
    let ty = match field {
        Field::Kept { ty, .. } => quote! { #ty },
        Field::Dropped { .. } => quote! { () },
    };
    quote! { ::tygr::__private::Memo<<#mode as ::tygr::Success>::Output<#ty>> }
}

/// Parse each field, then build the value; or, if `validated`, build it
/// whatever the mode and check it. A field in a memo slot (`slots`, by field)
/// is parsed once for every variant sharing it, and its value waits in the
/// slot for the variant that succeeds.
fn parse(model: &Model, variant: &Variant, slots: &[Option<usize>]) -> TokenStream {
    let mode = mode(model);
    let name = &model.name;
    let steps = variant
        .fields
        .iter()
        .zip(slots)
        .map(|(field, slot)| parse_field(field, &mode, *slot));
    let takes = variant
        .fields
        .iter()
        .zip(slots)
        .filter_map(|(field, slot)| {
            let (binding, slot) = (field.binding()?, Index::from((*slot)?));
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
/// position an earlier variant parsed it to, or fail as it failed, parsing
/// only if none did.
fn parse_field(field: &Field, mode: &TokenStream, slot: Option<usize>) -> TokenStream {
    let (binding, parse) = parse_expr(field, mode);
    let Some(slot) = slot else {
        return quote! { let (#binding, pos) = #parse?; };
    };
    let slot = Index::from(slot);
    quote! {
        let pos = match memo.#slot {
            ::tygr::__private::Memo::Parsed(_, end) => end,
            ::tygr::__private::Memo::Failed => return ::core::option::Option::None,
            ::tygr::__private::Memo::Untried => match #parse {
                ::core::option::Option::Some((value, end)) => {
                    memo.#slot = ::tygr::__private::Memo::Parsed(value, end);
                    end
                }
                ::core::option::Option::None => {
                    memo.#slot = ::tygr::__private::Memo::Failed;
                    return ::core::option::Option::None;
                }
            },
        };
    }
}

/// `field`'s binding, and the call parsing it in `mode`: as its type, or
/// converted; a dropped one is converted to `()`.
fn parse_expr(field: &Field, mode: &TokenStream) -> (TokenStream, TokenStream) {
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
