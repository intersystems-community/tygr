//! A derive's input, read into what the generated code needs.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Attribute, Data, DeriveInput, Fields, Generics, Ident, LitStr, Path, Type};

/// A derived `struct` or `enum` and its `#[tygr(...)]` options.
pub(crate) struct Model {
    pub(crate) ident: Ident,
    pub(crate) generics: Generics,
    /// The BNF rule name.
    pub(crate) name: String,
    /// Left out of BNF (and so inline too).
    pub(crate) hidden: bool,
    /// Spliced where it's referenced, rather than a rule reference.
    pub(crate) inline: bool,
    /// Checked with `Validate` after parsing.
    pub(crate) validated: bool,
    pub(crate) shape: Shape,
}

/// A sequence, or an alternation of sequences.
pub(crate) enum Shape {
    Struct(Variant),
    Enum(Vec<Variant>),
}

/// A struct, or one enum variant.
pub(crate) struct Variant {
    /// Builds it from its fields' bindings; also its pattern.
    pub(crate) constructor: TokenStream,
    pub(crate) fields: Vec<Field>,
}

/// One part of a sequence.
#[allow(clippy::large_enum_variant)] // Built once per field, at compile time.
pub(crate) enum Field {
    /// A field, parsed as its type or converted, bound as `binding`, and
    /// printed with its `Print` or `write_to`.
    Kept {
        binding: Ident,
        ty: Type,
        conversion: Option<Conversion>,
        write_to: Option<Path>,
    },
    /// A unit variant's source grammar: matched, then dropped.
    Dropped(Type),
}

/// `#[tygr(<kind> = source)]` on a field.
pub(crate) struct Conversion {
    pub(crate) kind: Kind,
    pub(crate) source: Type,
}

/// The std trait a field converts with.
#[derive(Clone, Copy)]
pub(crate) enum Kind {
    TryFrom,
    TryFromInfallible,
    FromStr,
    FromStrInfallible,
}

impl Model {
    pub(crate) fn new(input: &DeriveInput) -> syn::Result<Self> {
        let ident = &input.ident;
        let shape = match &input.data {
            Data::Struct(data) => {
                Shape::Struct(Variant::new(quote! { #ident }, &data.fields, &[])?)
            }
            Data::Enum(data) => Shape::Enum(
                data.variants
                    .iter()
                    .map(|variant| {
                        let path = &variant.ident;
                        Variant::new(quote! { #ident::#path }, &variant.fields, &variant.attrs)
                    })
                    .collect::<syn::Result<_>>()?,
            ),
            Data::Union(_) => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "Grammar cannot be derived for unions",
                ))
            }
        };
        let mut model = Model {
            ident: ident.clone(),
            generics: input.generics.clone(),
            name: ident.to_string(),
            hidden: false,
            inline: false,
            validated: false,
            shape,
        };
        for attr in tygr_attrs(&input.attrs) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    model.name = meta.value()?.parse::<LitStr>()?.value();
                } else if meta.path.is_ident("hidden") {
                    // Hidden nodes aren't in BNF, so they must be inline too.
                    model.hidden = true;
                    model.inline = true;
                } else if meta.path.is_ident("inline") {
                    model.inline = true;
                } else if meta.path.is_ident("validated") {
                    model.validated = true;
                } else {
                    return Err(meta.error("expected `name`, `hidden`, `inline`, or `validated`"));
                }
                Ok(())
            })?;
        }
        Ok(model)
    }

    /// The variants (a struct is one).
    pub(crate) fn variants(&self) -> &[Variant] {
        match &self.shape {
            Shape::Struct(variant) => std::slice::from_ref(variant),
            Shape::Enum(variants) => variants,
        }
    }

    /// The generics, each type parameter bounded by `bound`.
    pub(crate) fn bounded(&self, bound: TokenStream) -> Generics {
        let mut generics = self.generics.clone();
        for param in generics.type_params_mut() {
            param.bounds.push(syn::parse_quote!(#bound));
        }
        generics
    }
}

impl Variant {
    /// `path`'s fields, bound as `_f0`, `_f1`, … so no field name clashes
    /// with the generated code. Only a unit variant takes `attrs`.
    fn new(path: TokenStream, fields: &Fields, attrs: &[Attribute]) -> syn::Result<Self> {
        if let Fields::Unit = fields {
            let FieldAttrs {
                conversion,
                write_to,
            } = FieldAttrs::new(attrs)?;
            if let Some(write_to) = write_to {
                return Err(syn::Error::new_spanned(
                    write_to,
                    "a unit variant prints its source's default, not `write_to`",
                ));
            }
            let dropped = conversion.map(|conversion| Field::Dropped(conversion.source));
            return Ok(Variant {
                constructor: path,
                fields: dropped.into_iter().collect(),
            });
        }
        if let Some(attr) = tygr_attrs(attrs).next() {
            return Err(syn::Error::new_spanned(
                attr,
                "only a unit variant takes `#[tygr(...)]`; put it on a field",
            ));
        }
        let bindings: Vec<Ident> = (0..fields.len()).map(|i| format_ident!("_f{i}")).collect();
        let constructor = match fields {
            Fields::Named(named) => {
                let names = named.named.iter().map(|field| &field.ident);
                quote! { #path { #(#names: #bindings),* } }
            }
            _ => quote! { #path ( #(#bindings),* ) },
        };
        let fields = fields
            .iter()
            .zip(bindings)
            .map(|(field, binding)| {
                let FieldAttrs {
                    conversion,
                    write_to,
                } = FieldAttrs::new(&field.attrs)?;
                Ok(Field::Kept {
                    binding,
                    ty: field.ty.clone(),
                    conversion,
                    write_to,
                })
            })
            .collect::<syn::Result<_>>()?;
        Ok(Variant {
            constructor,
            fields,
        })
    }
}

impl Field {
    /// The grammar that parses it: its type, unless converted.
    pub(crate) fn source(&self) -> &Type {
        match self {
            Field::Kept {
                conversion: Some(conversion),
                ..
            } => &conversion.source,
            Field::Kept { ty, .. } | Field::Dropped(ty) => ty,
        }
    }

    /// Its binding, unless dropped.
    pub(crate) fn binding(&self) -> Option<&Ident> {
        match self {
            Field::Kept { binding, .. } => Some(binding),
            Field::Dropped(_) => None,
        }
    }

    /// Can its conversion fail?
    pub(crate) fn fallible(&self) -> bool {
        matches!(
            self,
            Field::Kept {
                conversion: Some(Conversion {
                    kind: Kind::TryFrom | Kind::FromStr,
                    ..
                }),
                ..
            }
        )
    }
}

impl Kind {
    const ALL: [Kind; 4] = [
        Kind::TryFrom,
        Kind::TryFromInfallible,
        Kind::FromStr,
        Kind::FromStrInfallible,
    ];

    /// Its keyword in `#[tygr(...)]`, also its `tygr::__private` helper's name.
    fn keyword(self) -> &'static str {
        match self {
            Kind::TryFrom => "try_from",
            Kind::TryFromInfallible => "try_from_infallible",
            Kind::FromStr => "from_str",
            Kind::FromStrInfallible => "from_str_infallible",
        }
    }

    /// The `tygr::__private` helper that parses and converts.
    pub(crate) fn helper(self) -> Ident {
        format_ident!("{}", self.keyword())
    }
}

/// A field's `#[tygr(...)]` options (a unit variant's, for its source).
#[derive(Default)]
struct FieldAttrs {
    conversion: Option<Conversion>,
    write_to: Option<Path>,
}

impl FieldAttrs {
    fn new(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut field = FieldAttrs::default();
        for attr in tygr_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("write_to") {
                    field.write_to = Some(meta.value()?.parse()?);
                } else if let Some(kind) = Kind::ALL
                    .into_iter()
                    .find(|kind| meta.path.is_ident(kind.keyword()))
                {
                    if field.conversion.is_some() {
                        return Err(meta.error("a field converts only one way"));
                    }
                    let source = meta.value()?.parse()?;
                    field.conversion = Some(Conversion { kind, source });
                } else {
                    return Err(meta.error(
                        "expected `try_from`, `try_from_infallible`, `from_str`, \
                         `from_str_infallible`, or `write_to`",
                    ));
                }
                Ok(())
            })?;
        }
        Ok(field)
    }
}

/// The `#[tygr(...)]` attributes among `attrs`.
fn tygr_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("tygr"))
}
