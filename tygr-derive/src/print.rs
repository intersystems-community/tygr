//! A derived `Print`.

use crate::model::{Conversion, Field, Kind, Model};
use proc_macro2::TokenStream;
use quote::quote;

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

/// Print `field`: a converted value back through its source, converted text
/// through `Display`, and a dropped source as its default.
fn print_field(field: &Field) -> TokenStream {
    match field {
        Field::Kept {
            binding,
            conversion: None,
            ..
        } => quote! { ::tygr::Print::print_to(#binding, buf); },
        Field::Kept {
            binding,
            ty,
            conversion: Some(Conversion { kind, source }),
        } => match kind {
            Kind::From | Kind::TryFrom => quote! {
                ::tygr::Print::print_to(&<#source as ::core::convert::From<&#ty>>::from(#binding), buf);
            },
            Kind::FromStr | Kind::FromStrInfallible => quote! {
                buf.extend_from_slice(::std::string::ToString::to_string(#binding).as_bytes());
            },
        },
        Field::Dropped(source) => quote! {
            ::tygr::Print::print_to(&<#source as ::core::default::Default>::default(), buf);
        },
    }
}
