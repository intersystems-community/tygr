//! The parsing half of a derived `Grammar`, and its `GrammarRule`.

use crate::dispatch::dispatch;
use crate::model::{Conversion, Field, Model, Shape, Variant};
use proc_macro2::TokenStream;
use quote::quote;

/// The `Grammar` and `GrammarRule` impls, given `print_to`.
pub(crate) fn expand(model: &Model, print_to: TokenStream) -> TokenStream {
    let Model {
        ident,
        name,
        hidden,
        inline,
        validated,
        shape,
        ..
    } = model;
    let generics = model.bounded(quote! { ::tygr::Grammar });
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
            let fail_ats = variants.iter().map(fail_at);
            let sequences = variants.iter().map(sequence);
            let (parse_at, items) = dispatch(&arms, &firsts);
            let inherent = quote! {
                impl #impl_generics #ident #ty_generics #where_clause {
                    #items

                    // Every variant reports (`&`, not `&&`); the enum is required only
                    // if all variants are.
                    #[inline]
                    fn fail_at_variants(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
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
                quote! { Self::fail_at_variants(pos, state) },
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
    let to_bnf = if *hidden {
        quote! { ::tygr::bnf::Expr::Hidden }
    } else if *inline {
        def.clone()
    } else {
        quote! { ::tygr::bnf::Expr::RuleRef(#name.to_string()) }
    };
    let (parse_at, fail_at) = (with_node(model, parse_at), with_node(model, fail_at));
    // `__Mode`, not `M`, so it can't shadow a type parameter of the user's.
    quote! {
        #inherent

        impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline(always)]
            fn parse_at<__Mode: ::tygr::Mode>(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(<__Mode as ::tygr::Mode>::Output<Self>, usize)> {
                #parse_at
            }

            #print_to

            fn to_bnf() -> ::tygr::bnf::Expr {
                #to_bnf
            }

            fn fail_at(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at
            }
        }

        impl #impl_generics ::tygr::GrammarRule for #ident #ty_generics #where_clause {
            const NAME: &'static str = #name;

            fn to_bnf_def() -> ::tygr::bnf::Expr {
                #def
            }
        }
    }
}

/// `body` in a trace node for this rule, unless it's inline.
fn with_node(model: &Model, body: TokenStream) -> TokenStream {
    if model.inline {
        body
    } else {
        quote! {
            let mut state = state.node(<Self as ::tygr::GrammarRule>::NAME, pos);
            #body
        }
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
        quote! { ::tygr::Parse }
    } else {
        quote! { __Mode }
    };
    let steps = variant.fields.iter().map(|field| parse_field(field, &mode));
    let constructor = &variant.constructor;
    if model.validated {
        return quote! {
            let start = pos;
            #(#steps)*
            let value = #constructor;
            if !::tygr::Validate::validate(&value) {
                state.reject(<Self as ::tygr::GrammarRule>::NAME, start..pos, <Self as ::tygr::Validate>::REQUIREMENT);
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

/// Parse `field` in `mode`, binding it; a dropped one is only matched.
fn parse_field(field: &Field, mode: &TokenStream) -> TokenStream {
    match field {
        Field::Kept {
            binding,
            ty,
            conversion: None,
        } => quote! {
            let (#binding, pos) = <#ty as ::tygr::Grammar>::parse_at::<#mode>(input, pos, state.reborrow())?;
        },
        Field::Kept {
            binding,
            ty,
            conversion: Some(Conversion { kind, source }),
        } => {
            let helper = kind.helper();
            // A fallible conversion records its rejections.
            let node = field
                .fallible()
                .then(|| quote! { <Self as ::tygr::GrammarRule>::NAME });
            quote! {
                let (#binding, pos) = ::tygr::__private::#helper::<#source, #ty, #mode>(input, pos, state.reborrow(), #node)?;
            }
        }
        Field::Dropped(source) => quote! {
            let ((), pos) = <#source as ::tygr::Grammar>::parse_at::<::tygr::Scan>(input, pos, state.reborrow())?;
        },
    }
}

/// `fail_at` for `variant`'s fields, stopping after the first required one.
fn fail_at(variant: &Variant) -> TokenStream {
    let sources = variant.fields.iter().map(Field::source);
    quote! {
        #(<#sources as ::tygr::Grammar>::fail_at(pos, state.reborrow()) ||)* false
    }
}

/// `variant`'s fields as a BNF sequence; a fallible conversion is footnoted.
fn sequence(variant: &Variant) -> TokenStream {
    let bnfs = variant.fields.iter().map(|field| {
        let source = field.source();
        let bnf = quote! { <#source as ::tygr::Grammar>::to_bnf() };
        if field.fallible() {
            quote! { ::tygr::bnf::Expr::side_condition(#bnf, ::tygr::__private::CONVERTIBLE) }
        } else {
            bnf
        }
    });
    quote! { ::tygr::bnf::Expr::sequence(vec![#(#bnfs),*]) }
}
