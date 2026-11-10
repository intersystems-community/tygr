//! A derive's input, read into what the generated code needs.

use proc_macro2::{TokenStream, TokenTree};
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
    /// printed with `print_to` if given, else its `Print` (a converted field
    /// needs `print_to`).
    Kept {
        binding: Ident,
        ty: Type,
        conversion: Option<Conversion>,
        print_to: Option<Path>,
    },
    /// A unit variant's source grammar, converted into `()`; printed with
    /// `print_to`, which takes `&()`.
    Dropped {
        kind: Kind,
        source: Type,
        print_to: Option<Path>,
    },
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
                    "a grammar is a struct or an enum, not a union",
                ));
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

    /// The fields, in every variant.
    pub(crate) fn fields(&self) -> impl Iterator<Item = &Field> {
        self.variants().iter().flat_map(|variant| &variant.fields)
    }

    /// What to `match` for `reference`'s variants: the value itself for an
    /// empty enum, as a reference is never empty.
    pub(crate) fn matched(&self, reference: TokenStream) -> TokenStream {
        if self.variants().is_empty() {
            quote! { *#reference }
        } else {
            reference
        }
    }

    /// The generics, with `bound` on each type parameter that `types` use.
    pub(crate) fn bounded<'a>(
        &self,
        bound: TokenStream,
        types: impl Iterator<Item = &'a Type>,
    ) -> Generics {
        let types: Vec<TokenStream> = types.map(|ty| quote! { #ty }).collect();
        let mut generics = self.generics.clone();
        for param in generics.type_params_mut() {
            if types.iter().any(|ty| mentions(ty.clone(), &param.ident)) {
                param.bounds.push(syn::parse_quote!(#bound));
            }
        }
        generics
    }
}

/// Does `tokens` use `ident`?
fn mentions(tokens: TokenStream, ident: &Ident) -> bool {
    tokens.into_iter().any(|token| match token {
        TokenTree::Ident(other) => other == *ident,
        TokenTree::Group(group) => mentions(group.stream(), ident),
        _ => false,
    })
}

impl Variant {
    /// `path`'s fields, bound as `_f0`, `_f1`, … so no field name clashes
    /// with the generated code. Only a unit variant takes `attrs`.
    fn new(path: TokenStream, fields: &Fields, attrs: &[Attribute]) -> syn::Result<Self> {
        if let Fields::Unit = fields {
            let FieldAttrs {
                conversion,
                print_to,
            } = FieldAttrs::new(attrs)?;
            let dropped = match (conversion, print_to) {
                (Some(Conversion { kind, source }), print_to) => {
                    if let Kind::FromStr | Kind::FromStrInfallible = kind {
                        return Err(syn::Error::new_spanned(
                            source,
                            "a unit variant converts its source into `()`, with `try_from` or \
                             `try_from_infallible`",
                        ));
                    }
                    Some(Field::Dropped {
                        kind,
                        source,
                        print_to,
                    })
                }
                (None, Some(print_to)) => {
                    return Err(syn::Error::new_spanned(
                        print_to,
                        "`print_to` on a unit variant needs a source, e.g. `try_from_infallible = S`; \
                         without one it matches the empty text, with nothing to print",
                    ));
                }
                (None, None) => None,
            };
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
                    print_to,
                } = FieldAttrs::new(&field.attrs)?;
                Ok(Field::Kept {
                    binding,
                    ty: field.ty.clone(),
                    conversion,
                    print_to,
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
            Field::Kept { ty, .. } | Field::Dropped { source: ty, .. } => ty,
        }
    }

    /// The type it prints with `Print`: its own, unless converted or
    /// `print_to` prints it.
    pub(crate) fn printed(&self) -> Option<&Type> {
        match self {
            Field::Kept {
                ty,
                conversion: None,
                print_to: None,
                ..
            } => Some(ty),
            _ => None,
        }
    }

    /// The source of a converted field that nothing prints: converting
    /// loses the text, so only `print_to` can print it back.
    pub(crate) fn unprintable(&self) -> Option<&Type> {
        match self {
            Field::Kept {
                conversion: Some(Conversion { source, .. }),
                print_to: None,
                ..
            }
            | Field::Dropped {
                source,
                print_to: None,
                ..
            } => Some(source),
            _ => None,
        }
    }

    /// Its binding, unless dropped.
    pub(crate) fn binding(&self) -> Option<&Ident> {
        match self {
            Field::Kept { binding, .. } => Some(binding),
            Field::Dropped { .. } => None,
        }
    }

    /// What its text must do if its conversion can fail, in errors and BNF:
    /// be convertible to its type.
    pub(crate) fn requirement(&self) -> Option<String> {
        let (kind, ty) = match self {
            Field::Kept {
                ty,
                conversion: Some(Conversion { kind, .. }),
                ..
            } => (kind, written(ty)),
            Field::Dropped { kind, .. } => (kind, "()".to_string()),
            Field::Kept {
                conversion: None, ..
            } => return None,
        };
        matches!(kind, Kind::TryFrom | Kind::FromStr).then(|| format!("be convertible to {ty}"))
    }
}

/// `ty` as written: `Small<u8>`, not the `Small < u8 >` its tokens print as.
fn written(ty: &Type) -> String {
    let spaced: Vec<char> = quote! { #ty }.to_string().chars().collect();
    let word = |ch: Option<&char>| ch.is_some_and(|&ch| ch.is_alphanumeric() || "_'".contains(ch));
    let mut written = String::new();
    for (i, &ch) in spaced.iter().enumerate() {
        // Keep a space between words (`dyn Fn`) and after `,` or `;`.
        let before = i.checked_sub(1).and_then(|i| spaced.get(i));
        if ch != ' '
            || (word(before) && word(spaced.get(i + 1)))
            || before.is_some_and(|ch| ",;".contains(*ch))
        {
            written.push(ch);
        }
    }
    written
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
    print_to: Option<Path>,
}

impl FieldAttrs {
    fn new(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut field = FieldAttrs::default();
        for attr in tygr_attrs(attrs) {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("print_to") {
                    if field.print_to.is_some() {
                        return Err(meta.error("a field prints only one way"));
                    }
                    field.print_to = Some(meta.value()?.parse()?);
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
                         `from_str_infallible`, or `print_to`",
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
