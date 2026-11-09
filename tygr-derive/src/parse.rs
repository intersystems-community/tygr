//! A derived `Parse`.

use crate::dispatch::dispatch;
use crate::model::{Conversion, Field, Model, Shape, Variant};
use proc_macro2::TokenStream;
use quote::quote;

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
    // Only the parameters it parses need `Parse`, but all need `'static`.
    let mut generics = model.bounded(quote! { ::tygr::Parse }, model.fields().map(Field::source));
    for param in generics.type_params_mut() {
        param.bounds.push(syn::parse_quote!('static));
    }
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let (first, parse_at, fail_at, def, inherent) = match shape {
        Shape::Struct(variant) => (
            first(variant),
            parse(model, variant),
            fail_at(variant),
            sequence(variant),
            quote! {},
        ),
        Shape::Enum(variants) => {
            let firsts: Vec<_> = variants.iter().map(first).collect();
            let arms: Vec<_> = variants
                .iter()
                .map(|variant| parse(model, variant))
                .collect();
            let fail_ats: Vec<_> = variants.iter().map(fail_at).collect();
            let sequences = variants.iter().map(sequence);
            let (parse_at, items) = dispatch(&arms, &firsts, &fail_ats);
            let inherent = quote! {
                impl #impl_generics #ident #ty_generics #where_clause {
                    #items

                    // Every variant reports (`&`, not `&&`); the enum is required only
                    // if all variants are.
                    #[inline]
                    fn __fail_at_variants(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                        #((#fail_ats) &)* true
                    }
                }
            };
            (
                firsts.iter().fold(
                    quote! { ::tygr::first::Required<::tygr::first::byte_set::Emp> },
                    |first, variant| quote! { <#first as ::tygr::first::First>::Alt<#variant> },
                ),
                parse_at,
                quote! { Self::__fail_at_variants(pos, state) },
                quote! { ::tygr::bnf::Expr::alternation(vec![#(#sequences),*]) },
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
            quote! { ::tygr::bnf::Expr::RuleRef(#name.to_string()) },
            quote! {
                if rules.insert::<Self>(#name, || #def) {
                    #visit_parts
                }
            },
        )
    };
    let (parse_at, fail_at) = (with_node(model, parse_at), with_node(model, fail_at));
    // `__Mode`, not `M`, so it can't shadow a type parameter of the user's.
    quote! {
        #inherent

        unsafe impl #impl_generics ::tygr::Parse for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline(always)]
            fn parse_at<__Mode: ::tygr::Mode>(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(<__Mode as ::tygr::Mode>::Output<Self>, usize)> {
                #parse_at
            }

            fn fail_at(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #to_bnf
            }

            fn visit_rules(#[allow(unused_variables)] rules: &mut ::tygr::bnf::Rules) {
                #visit_rules
            }
        }
    }
}

/// `body` in a trace node for this rule, unless it's inline.
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
        quote! { ::tygr::first::Optional<::tygr::first::byte_set::Emp> },
        |first, source| quote! { <#first as ::tygr::first::First>::Seq<#source> },
    )
}

/// Parse each field, then build the value; or, if `validated`, build it
/// whatever the mode and check it.
fn parse(model: &Model, variant: &Variant) -> TokenStream {
    // Validating needs the value, so then the fields always build.
    let mode = if model.validated {
        quote! { ::tygr::Build }
    } else {
        quote! { __Mode }
    };
    let name = &model.name;
    let steps = variant
        .fields
        .iter()
        .map(|field| parse_field(field, &mode, name));
    let constructor = &variant.constructor;
    if model.validated {
        return quote! {
            let start = pos;
            #(#steps)*
            let value = #constructor;
            if !::tygr::Validate::validate(&value) {
                state.reject(#name, start..pos, <Self as ::tygr::Validate>::REQUIREMENT);
                return None;
            }
            Some((<__Mode as ::tygr::Mode>::build(|_| value), pos))
        };
    }
    let bindings: Vec<_> = variant.fields.iter().filter_map(Field::binding).collect();
    let mode = if bindings.is_empty() {
        quote! { _ }
    } else {
        quote! { mode }
    };
    quote! {
        #(#steps)*
        Some((<__Mode as ::tygr::Mode>::build(|#mode| {
            #(let #bindings = <__Mode as ::tygr::Mode>::get(mode, #bindings);)*
            #constructor
        }), pos))
    }
}

/// Parse `field` in `mode`, binding it; a dropped one is only matched. A
/// failed conversion is rule `name`'s rejection.
fn parse_field(field: &Field, mode: &TokenStream, name: &str) -> TokenStream {
    match field {
        Field::Kept {
            binding,
            ty,
            conversion: None,
            ..
        } => quote! {
            let (#binding, pos) = <#ty as ::tygr::Parse>::parse_at::<#mode>(input, pos, state.reborrow())?;
        },
        Field::Kept {
            binding,
            ty,
            conversion: Some(Conversion { kind, source }),
            ..
        } => {
            let helper = kind.helper();
            let node = field.fallible().then_some(name);
            quote! {
                let (#binding, pos) = ::tygr::__private::#helper::<#source, #ty, #mode>(input, pos, state.reborrow(), #node)?;
            }
        }
        Field::Dropped(source) => quote! {
            let ((), pos) = ::tygr::__private::dropped::<#source>(input, pos, state.reborrow())?;
        },
    }
}

/// `fail_at` for `variant`'s fields, stopping after the first required one.
fn fail_at(variant: &Variant) -> TokenStream {
    let sources = variant.fields.iter().map(Field::source);
    quote! {
        #(<#sources as ::tygr::Parse>::fail_at(pos, state.reborrow()) ||)* false
    }
}

/// `variant`'s fields as a BNF sequence; a fallible conversion is footnoted.
fn sequence(variant: &Variant) -> TokenStream {
    let bnfs = variant.fields.iter().map(|field| {
        let source = field.source();
        let bnf = quote! { <#source as ::tygr::Parse>::to_bnf() };
        if field.fallible() {
            quote! { ::tygr::bnf::Expr::side_condition(#bnf, ::tygr::__private::CONVERTIBLE) }
        } else {
            bnf
        }
    });
    quote! { ::tygr::bnf::Expr::sequence(vec![#(#bnfs),*]) }
}

/// Reads every field, as a derived `Print` would: a grammar's fields are
/// its syntax, so the dead-code lint shouldn't flag them as never read.
pub(crate) fn read_fields(model: &Model) -> TokenStream {
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
