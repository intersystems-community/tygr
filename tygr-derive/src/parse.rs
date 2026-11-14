//! A derived `Parse`.

use crate::dispatch::dispatch;
use crate::model::{Conversion, Field, Model, Shape, Variant};
use proc_macro2::TokenStream;
use quote::{quote, quote_spanned};

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
            parse(model, variant),
            sequence(variant),
            None,
        ),
        Shape::Enum(variants) => {
            let firsts: Vec<_> = variants.iter().map(first).collect();
            let arms: Vec<_> = variants
                .iter()
                .map(|variant| parse(model, variant))
                .collect();
            let sequences = variants.iter().map(sequence);
            let (parse_at, items) = dispatch(&arms, &firsts);
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

/// Parse each field, then build the value; or, if `validated`, build it
/// whatever the mode and check it.
fn parse(model: &Model, variant: &Variant) -> TokenStream {
    let mode = if model.validated {
        quote! { ::tygr::Value }
    } else {
        quote! { __Success }
    };
    let name = &model.name;
    let steps = variant.fields.iter().map(|field| parse_field(field, &mode));
    let constructor = &variant.constructor;
    let bindings: Vec<_> = variant.fields.iter().filter_map(Field::binding).collect();
    if model.validated {
        return quote! {
            let start = pos;
            #(#steps)*
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
        ::core::option::Option::Some((#output, pos))
    }
}

/// Parse `field` in `mode`, binding it; a dropped one is converted to `()`.
fn parse_field(field: &Field, mode: &TokenStream) -> TokenStream {
    let (binding, ty, kind) = match field {
        Field::Kept {
            binding,
            ty,
            conversion: None,
            ..
        } => {
            return quote! {
                let (#binding, pos) = <#ty as ::tygr::Parse>::parse_at::<#mode, __Level>(input, pos, state.reborrow())?;
            };
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
    quote! {
        let (#binding, pos) = ::tygr::__private::#helper::<#source, #ty, #mode, __Level>(input, pos, state.reborrow(), #target)?;
    }
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
