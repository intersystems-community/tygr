//! A derived `Print`.

use crate::model::{Field, Model};
use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::Ident;

/// The `Print` impl, printing each field in turn.
pub(crate) fn expand(model: &Model) -> TokenStream {
    let ident = &model.ident;
    let generics = model.bounded(quote! { ::tygr::Print });
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let variants = model.variants();
    let constructors = variants.iter().map(|variant| &variant.constructor);
    let prints = variants.iter().map(|variant| {
        let fields = variant.fields.iter().map(print_field);
        quote! { #(#fields)* }
    });
    quote! {
        impl #impl_generics ::tygr::Print for #ident #ty_generics #where_clause {
            fn print_to(&self, #[allow(unused_variables)] buf: &mut ::std::vec::Vec<u8>) {
                match self {
                    #(#constructors => { #prints })*
                }
            }
        }
    }
}

/// Print `field` with its `write_to`, or else its `Print`; a dropped source
/// prints as its default.
fn print_field(field: &Field) -> TokenStream {
    match field {
        Field::Kept {
            binding,
            write_to: Some(write_to),
            ..
        } => quote! { #write_to(#binding, buf); },
        Field::Kept { binding, ty, .. } => {
            // At the type's span, so a type that doesn't print is blamed.
            let binding = Ident::new(&binding.to_string(), ty.span());
            quote! { ::tygr::Print::print_to(#binding, buf); }
        }
        Field::Dropped(source) => quote! {
            ::tygr::Print::print_to(&<#source as ::core::default::Default>::default(), buf);
        },
    }
}
