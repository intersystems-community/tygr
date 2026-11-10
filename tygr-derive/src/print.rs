//! A derived `Print`.

use crate::model::{Field, Model};
use proc_macro2::TokenStream;
use quote::quote;
use syn::Ident;
use syn::spanned::Spanned;

/// The `Print` impl, printing each field in turn; a compile error if a
/// converted field has no `print_to`.
pub(crate) fn expand(model: &Model) -> TokenStream {
    let ident = &model.ident;
    let generics = model.bounded(
        quote! { ::tygr::Print },
        model.fields().filter_map(Field::printed),
    );
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let mut unprintable = model.fields().filter_map(Field::unprintable).map(|source| {
        syn::Error::new_spanned(
            source,
            "a converted field needs `print_to = f` to print (`f: fn(&T, &mut Vec<u8>)`, e.g. \
             `print_display`; `&()` for a unit variant), or derive only `Parse`",
        )
    });
    if let Some(mut error) = unprintable.next() {
        unprintable.for_each(|other| error.combine(other));
        let error = error.into_compile_error();
        // With a stub impl, so grammars using this one don't fail to print too.
        return quote! {
            #error
            impl #impl_generics ::tygr::Print for #ident #ty_generics #where_clause {
                fn print_to(&self, _: &mut ::std::vec::Vec<u8>) {}
            }
        };
    }
    let variants = model.variants();
    let constructors = variants.iter().map(|variant| &variant.constructor);
    let prints = variants.iter().map(|variant| {
        let fields = variant.fields.iter().map(print_field);
        quote! { #(#fields)* }
    });
    let value = model.matched(quote! { self });
    quote! {
        impl #impl_generics ::tygr::Print for #ident #ty_generics #where_clause {
            fn print_to(&self, #[allow(unused_variables)] buf: &mut ::std::vec::Vec<u8>) {
                match #value {
                    #(#constructors => { #prints })*
                }
            }
        }
    }
}

/// Print `field` with its `print_to` (a dropped source's takes `&()`), or
/// else its `Print`.
fn print_field(field: &Field) -> TokenStream {
    match field {
        Field::Kept {
            binding,
            print_to: Some(print_to),
            ..
        } => quote! { #print_to(#binding, buf); },
        Field::Dropped {
            print_to: Some(print_to),
            ..
        } => quote! { #print_to(&(), buf); },
        Field::Kept {
            binding,
            ty,
            conversion: None,
            print_to: None,
        } => {
            // At the type's span, so a type that doesn't print is blamed.
            let binding = Ident::new(&binding.to_string(), ty.span());
            quote! { ::tygr::Print::print_to(#binding, buf); }
        }
        Field::Kept { .. } | Field::Dropped { .. } => {
            unreachable!("`expand` rejects a converted field without `print_to`")
        }
    }
}
