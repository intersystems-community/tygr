//! The printing half of a derived `Grammar`.

use crate::model::{Conversion, Field, Kind, Model};
use proc_macro2::TokenStream;
use quote::quote;

/// `print_to`, printing each field in turn. Without the `print` feature, it
/// panics, but still reads the fields so they count as read.
pub(crate) fn print_to(model: &Model) -> TokenStream {
    let variants = model.variants();
    let constructors = variants.iter().map(|variant| &variant.constructor);
    if !cfg!(feature = "print") {
        return quote! {
            fn print_to(&self, _buf: &mut ::std::vec::Vec<u8>) {
                match self { #(#constructors => {})* }
                unimplemented!("tygr's `print` feature is off")
            }
        };
    }
    let prints = variants.iter().map(|variant| {
        let fields = variant.fields.iter().map(print_field);
        quote! { #(#fields)* }
    });
    quote! {
        fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
            match self {
                #(#constructors => { #prints })*
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
        } => quote! { ::tygr::Grammar::print_to(#binding, buf); },
        Field::Kept {
            binding,
            ty,
            conversion: Some(Conversion { kind, source }),
        } => match kind {
            Kind::From | Kind::TryFrom => quote! {
                ::tygr::Grammar::print_to(&<#source as ::core::convert::From<&#ty>>::from(#binding), buf);
            },
            Kind::FromStr | Kind::FromStrInfallible => quote! {
                buf.extend_from_slice(::std::string::ToString::to_string(#binding).as_bytes());
            },
        },
        Field::Dropped(source) => quote! {
            ::tygr::Grammar::print_to(&<#source as ::core::default::Default>::default(), buf);
        },
    }
}
