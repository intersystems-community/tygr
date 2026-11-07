use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, Data, DataEnum, DataStruct, DeriveInput, Fields, Generics, Ident};

/// Derive `Grammar` (and `GrammarRule`) for a `struct` or `enum`.
///
/// A `struct` is a sequence of its fields; an `enum` an alternation of its
/// variants. `#[grammar(name = "…", hidden, inline, validated)]` are
/// optional (see `tygr`'s crate docs).
#[proc_macro_derive(Grammar, attributes(grammar))]
pub fn derive_grammar(input: TokenStream) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    for param in input.generics.type_params_mut() {
        param.bounds.push(parse_quote!(::tygr::Grammar));
    }
    match impl_grammar(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// `Grammar` by parsing `GrammarFrom::Source` and calling
/// [`FromStr`](std::str::FromStr) on the matched text; `Err` backtracks.
/// Takes the same `#[grammar(...)]` options as `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromStr, attributes(grammar))]
pub fn derive_grammar_from_str(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::FromStr)
}

/// `Grammar` by parsing `GrammarFrom::Source` and calling
/// [`FromStr`](std::str::FromStr) on the matched text, whose `Err` must be
/// [`Infallible`](std::convert::Infallible). It can't fail, so scanning skips
/// the conversion. Takes the same `#[grammar(...)]` options as
/// `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromStrInfallible, attributes(grammar))]
pub fn derive_grammar_from_str_infallible(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::FromStrInfallible)
}

/// `Grammar` by parsing `GrammarFrom::Source` and converting with
/// [`From`]. Takes the same `#[grammar(...)]` options as `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromOther, attributes(grammar))]
pub fn derive_grammar_from_source(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::From)
}

/// `Grammar` by parsing `GrammarFrom::Source` and converting with
/// [`TryFrom`]; `Err` backtracks. Takes the same `#[grammar(...)]` options as
/// `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarTryFromOther, attributes(grammar))]
pub fn derive_grammar_try_from_source(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::TryFrom)
}

#[derive(Clone, Copy)]
enum Convert {
    FromStr,
    FromStrInfallible,
    From,
    TryFrom,
}

fn derive_convert(input: TokenStream, convert: Convert) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    for param in input.generics.type_params_mut() {
        param.bounds.push(parse_quote!(::tygr::Grammar));
    }
    match impl_convert(&input, convert) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// `Token<G>`, `G` holding each char's UTF-8 bytes as `ByteEq`s.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEq(input: TokenStream) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    if value.is_empty() {
        return empty_literal_error(lit.span(), "StringEq");
    }
    let tuple = right_nested(value.chars().map(char_eq), pair);
    quote! { ::tygr::__private::Token<#tuple> }.into()
}

/// Like [`StringEq!`], each char an `Either` over its case variants.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEqCI(input: TokenStream) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    if value.is_empty() {
        return empty_literal_error(lit.span(), "StringEqCI");
    }
    let tuple = right_nested(value.chars().map(char_eq_ci), pair);
    quote! { ::tygr::__private::Token<#tuple> }.into()
}

fn empty_literal_error(span: proc_macro2::Span, macro_name: &str) -> TokenStream {
    syn::Error::new(
        span,
        format!("{macro_name}!() requires a non-empty literal"),
    )
    .to_compile_error()
    .into()
}

/// `ch`, then its Unicode case mappings longest first, so ordered choice
/// never stops at a shorter prefix (no char prefixes its own mappings).
fn case_variants(ch: char) -> Vec<String> {
    let me = ch.to_string();
    let mut others: Vec<String> = vec![ch.to_lowercase().collect(), ch.to_uppercase().collect()];
    others.retain(|other| *other != me);
    others.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    others.dedup();
    let mut variants = vec![me];
    variants.extend(others);
    variants
}

/// `ch`'s UTF-8 bytes as a tuple of `ByteEq`s.
fn char_eq(ch: char) -> TokenStream2 {
    byte_tuple(ch.to_string().as_bytes())
}

/// An `Either` over `ch`'s case variants, or its own tuple if it has none.
fn char_eq_ci(ch: char) -> TokenStream2 {
    let variants = case_variants(ch)
        .into_iter()
        .map(|variant| byte_tuple(variant.as_bytes()));
    right_nested(
        variants,
        |first, rest| quote! { ::tygr::__private::Either<#first, #rest> },
    )
}

/// `ByteEq`s for non-empty `bytes`, nested as pairs.
fn byte_tuple(bytes: &[u8]) -> TokenStream2 {
    right_nested(
        bytes
            .iter()
            .map(|byte| quote! { ::tygr::__private::ByteEq<#byte> }),
        pair,
    )
}

/// `(first, rest)`.
fn pair(first: TokenStream2, rest: TokenStream2) -> TokenStream2 {
    quote! { (#first, #rest) }
}

/// `[a, b, c]` → `join(a, join(b, c))`.
fn right_nested(
    items: impl DoubleEndedIterator<Item = TokenStream2>,
    join: impl Fn(TokenStream2, TokenStream2) -> TokenStream2,
) -> TokenStream2 {
    let mut items = items.rev();
    let last = items.next().expect("non-empty");
    items.fold(last, |rest, item| join(item, rest))
}

#[derive(Clone)]
struct Tag {
    name: TokenStream2,
    case: Option<TokenStream2>,
}

impl Tag {
    fn new(name: TokenStream2, case: Option<TokenStream2>) -> Self {
        Self { name, case }
    }

    fn as_constructor(&self) -> TokenStream2 {
        let name = &self.name;
        let case = if let Some(case) = &self.case {
            quote! { :: #case }
        } else {
            quote! {}
        };
        quote! { #name #case }
    }
}

fn with_node(inline: bool, body: TokenStream2) -> TokenStream2 {
    if inline {
        body
    } else {
        quote! {
            let mut state = state.node(<Self as ::tygr::GrammarRule>::NAME, pos);
            #body
        }
    }
}

/// `default_name` as a string literal, for `tygr::char_class!`.
#[doc(hidden)]
#[proc_macro]
pub fn __default_name(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as Ident);
    let name = default_name(&ident);
    quote! { #name }.into()
}

/// The BNF rule name when `#[grammar(name = "...")]` isn't given.
fn default_name(ident: &Ident) -> String {
    let ident = ident.to_string();
    #[cfg(all(feature = "lower_bnf_name", not(feature = "upper_bnf_name")))]
    let ident = ident.to_ascii_lowercase();
    #[cfg(all(feature = "upper_bnf_name", not(feature = "lower_bnf_name")))]
    let ident = ident.to_ascii_uppercase();
    ident
}

fn impl_grammar(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let ident = &input.ident;
    let generics = &input.generics;
    let GrammarAttr {
        name,
        hidden,
        inline,
        validated,
    } = grammar_attr(input)?;
    let name = name.unwrap_or_else(|| default_name(ident));
    match &input.data {
        Data::Struct(data) => impl_struct(ident, generics, name, hidden, inline, validated, data),
        Data::Enum(data) => impl_enum(ident, generics, name, hidden, inline, validated, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            ident,
            "Grammar cannot be derived for unions",
        )),
    }
}

fn grammar_attr(input: &DeriveInput) -> syn::Result<GrammarAttr> {
    for attr in &input.attrs {
        if attr.path().is_ident("grammar") {
            return attr.parse_args::<GrammarAttr>();
        }
    }
    Ok(GrammarAttr::default())
}

#[derive(Default)]
struct GrammarAttr {
    name: Option<String>,
    hidden: bool,
    inline: bool,
    validated: bool,
}

impl syn::parse::Parse for GrammarAttr {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attr = GrammarAttr::default();

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            if ident == "name" {
                let _: syn::Token![=] = input.parse()?;
                let lit: syn::LitStr = input.parse()?;
                attr.name = Some(lit.value());
            } else if ident == "hidden" {
                attr.hidden = true;
                // Hidden nodes aren't in BNF, so they must be inline too.
                attr.inline = true;
            } else if ident == "inline" {
                attr.inline = true;
            } else if ident == "validated" {
                attr.validated = true;
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `name`, `hidden`, `inline`, or `validated`",
                ));
            }
            if input.is_empty() {
                break;
            }
            let _: syn::Token![,] = input.parse()?;
        }

        Ok(attr)
    }
}

type ProcessedFields<'a> = Vec<(String, Ident, &'a syn::Type)>;

fn components_and_xts(tag: TokenStream2, fields: &Fields) -> (TokenStream2, ProcessedFields<'_>) {
    match fields {
        Fields::Named(fields) => {
            let fields: Vec<_> = fields
                .named
                .iter()
                .map(|field| {
                    let x = field.ident.as_ref().unwrap().clone();
                    let t = &field.ty;
                    (x.to_string(), x, t)
                })
                .collect();
            let xs = fields.iter().map(|(_, x, _)| x);
            (quote! { #tag { #(#xs),* } }, fields)
        }
        Fields::Unnamed(fields) => {
            let fields: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let x = Ident::new(&format!("_f{i}"), Span::call_site());
                    let t = &field.ty;
                    (i.to_string(), x, t)
                })
                .collect();
            let xs = fields.iter().map(|(_, x, _)| x);
            (quote! { #tag ( #(#xs),* ) }, fields)
        }
        Fields::Unit => (quote! { #tag }, vec![]),
    }
}

fn parse_at(validated: bool, fields: &ProcessedFields, constructor: &TokenStream2) -> TokenStream2 {
    let steps: Vec<_> = fields
        .iter()
        .map(|(_, x, t)| {
            quote! {
                let (#x, pos) = <#t as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            }
        })
        .collect();
    let (start_pos, validate) = if validated {
        let trace = if cfg!(feature = "trace") {
            quote! {
                state.expect(pos, ::tygr::Expectation::Rejected {
                    node: <Self as ::tygr::GrammarRule>::NAME,
                    range: start_pos..pos,
                    requirement: <Self as ::tygr::Validate>::REQUIREMENT,
                });
            }
        } else if cfg!(feature = "trace_pos") {
            quote! {
                state.expect(pos);
            }
        } else {
            quote! {}
        };
        (
            quote! { let start_pos = pos; },
            quote! {
                if !::tygr::Validate::validate(&value) {
                    #trace
                    return None
                }
            },
        )
    } else {
        (quote! {}, quote! {})
    };
    quote! {
        #start_pos
        #(#steps)*
        let value = #constructor;
        #validate
        Some((value, pos))
    }
}

fn scan_at(validated: bool, fields: &ProcessedFields, constructor: &TokenStream2) -> TokenStream2 {
    if validated {
        let parse_at = parse_at(true, fields, constructor);
        quote! { ({#parse_at}).map(|(_, pos)| pos) }
    } else {
        let steps: Vec<_> = fields
            .iter()
            .map(|(_, _, t)| {
                quote! {
                    let pos = <#t as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?;
                }
            })
            .collect();
        quote! {
            #(#steps)*
            Some(pos)
        }
    }
}

fn print_steps(fields: &ProcessedFields) -> Vec<TokenStream2> {
    fields
        .iter()
        .map(|(_, x, _)| {
            quote! {
                ::tygr::Grammar::print_to(#x, buf);
            }
        })
        .collect()
}

fn to_bnf(fields: &ProcessedFields) -> TokenStream2 {
    let ts = fields.iter().map(|(_, _, t)| quote! { #t });
    quote! {
        ::tygr::bnf::Expr::sequence(vec![
            #(<#ts as ::tygr::Grammar>::to_bnf()),*
        ])
    }
}

/// `A::fail_at(..) || B::fail_at(..) || .. || false`: stops after the first
/// required field.
fn fail_at(fields: &ProcessedFields) -> TokenStream2 {
    let calls = fields.iter().map(|(_, _, t)| {
        quote! { <#t as ::tygr::Grammar>::fail_at(pos, state.reborrow()) }
    });
    quote! {
        #(#calls ||)* false
    }
}

fn bnf_ref(grammar_name: &str, hidden: bool, to_bnf: &TokenStream2, inline: bool) -> TokenStream2 {
    if hidden {
        quote! { ::tygr::bnf::Expr::empty() }
    } else if inline {
        quote! { #to_bnf }
    } else {
        quote! { ::tygr::bnf::Expr::RuleRef(#grammar_name.to_string()) }
    }
}

struct FieldsInfo {
    constructor: TokenStream2,
    parse_at: TokenStream2,
    scan_at: TokenStream2,
    print_to: TokenStream2,
    to_bnf: TokenStream2,
    fail_at: TokenStream2,
    first: TokenStream2,
}

impl FieldsInfo {
    fn from(validated: bool, tag: Tag, fields: &Fields) -> Self {
        let (constructor, fields) = components_and_xts(tag.as_constructor(), fields);
        let print_steps = print_steps(&fields);
        let parse_at = parse_at(validated, &fields, &constructor);
        let scan_at = scan_at(validated, &fields, &constructor);
        let mut first = quote! { ::tygr::first::Optional<::tygr::first::byte_set::Emp> };
        for (_, _, t) in &fields {
            first = quote! {
                <#first as ::tygr::first::First>::Seq<#t>
            }
        }
        Self {
            parse_at: quote! { #parse_at },
            scan_at: quote! { #scan_at },
            print_to: quote! {#(#print_steps)*},
            to_bnf: to_bnf(&fields),
            fail_at: fail_at(&fields),
            constructor,
            first,
        }
    }
}

/// The requirement a `GrammarFrom*` conversion adds, in BNF and errors.
const CONVERTIBLE: &str = "be convertible";

fn impl_convert(input: &DeriveInput, convert: Convert) -> syn::Result<TokenStream2> {
    let ident = &input.ident;
    let generics = &input.generics;
    let GrammarAttr {
        name,
        hidden,
        inline,
        validated,
    } = grammar_attr(input)?;
    let name = name.unwrap_or_else(|| default_name(ident));
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty = quote! { #ident #ty_generics };
    let source = quote! { <#self_ty as ::tygr::GrammarFrom>::Source };
    let trace = if cfg!(feature = "trace") {
        quote! {
            state.expect(
                end,
                ::tygr::Expectation::Rejected {
                    node: <Self as ::tygr::GrammarRule>::NAME,
                    range: pos..end,
                    requirement: #CONVERTIBLE,
                },
            );
        }
    } else if cfg!(feature = "trace_pos") {
        quote! {
            state.expect(end);
        }
    } else {
        quote! {}
    };
    // `validated` can still reject a converted value.
    let validate = if validated {
        let validate_trace = if cfg!(feature = "trace") {
            quote! {
                state.expect(end, ::tygr::Expectation::Rejected {
                    node: <Self as ::tygr::GrammarRule>::NAME,
                    range: pos..end,
                    requirement: <Self as ::tygr::Validate>::REQUIREMENT,
                });
            }
        } else if cfg!(feature = "trace_pos") {
            quote! {
                state.expect(end);
            }
        } else {
            quote! {}
        };
        quote! {
            if !::tygr::Validate::validate(&value) {
                #validate_trace
                return None
            }
        }
    } else {
        quote! {}
    };
    let parse_at = match convert {
        Convert::FromStr => quote! {
            let end = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?;
            // SAFETY: see the invariant on `Grammar::parse_at`.
            let text = unsafe { ::core::str::from_utf8_unchecked(&input[pos..end]) };
            match <#self_ty as ::core::str::FromStr>::from_str(text) {
                Ok(value) => {
                    #validate
                    Some((value, end))
                }
                Err(err) => {
                    #trace
                    let _ = &err;
                    None
                }
            }
        },
        Convert::FromStrInfallible => quote! {
            let end = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?;
            // SAFETY: see the invariant on `Grammar::parse_at`.
            let text = unsafe { ::core::str::from_utf8_unchecked(&input[pos..end]) };
            let result: ::core::result::Result<#self_ty, ::core::convert::Infallible> =
                <#self_ty as ::core::str::FromStr>::from_str(text);
            let Ok(value) = result;
            #validate
            Some((value, end))
        },
        Convert::From => quote! {
            let (source, end) = <#source as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            let value = <#self_ty as ::core::convert::From<#source>>::from(source);
            #validate
            Some((value, end))
        },
        Convert::TryFrom => quote! {
            let (source, end) = <#source as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            match <#self_ty as ::core::convert::TryFrom<#source>>::try_from(source) {
                Ok(value) => {
                    #validate
                    Some((value, end))
                }
                Err(err) => {
                    #trace
                    let _ = &err;
                    None
                }
            }
        },
    };
    // Only infallible, unvalidated conversions can scan without converting.
    let scan_at = if !validated && matches!(convert, Convert::From | Convert::FromStrInfallible) {
        quote! {
            <#source as ::tygr::Grammar>::scan_at(input, pos, state)
        }
    } else {
        quote! {
            Self::parse_at(input, pos, state).map(|(_, end)| end)
        }
    };
    let to_bnf = quote! { <#source as ::tygr::Grammar>::to_bnf() };
    let bnf_ref = bnf_ref(&name, hidden, &to_bnf, inline);
    // Side-conditions only on the rule's own definition, not references.
    let to_bnf_def = match convert {
        Convert::From | Convert::FromStrInfallible => to_bnf,
        Convert::FromStr | Convert::TryFrom => quote! {
            ::tygr::bnf::Expr::side_condition(#to_bnf, #CONVERTIBLE)
        },
    };
    let to_bnf_def = if validated {
        quote! { ::tygr::bnf::Expr::side_condition(#to_bnf_def, <Self as ::tygr::Validate>::REQUIREMENT) }
    } else {
        to_bnf_def
    };
    Ok(quote! {
        impl #impl_generics ::tygr::Grammar for #self_ty #where_clause {
            type First = <#source as ::tygr::Grammar>::First;

            #[inline(always)]
            fn parse_at(input: &[u8], pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_at
            }

            #[inline(always)]
            fn scan_at(input: &[u8], pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_at
            }

            fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                <#self_ty as ::tygr::GrammarFrom>::print_to(self, buf);
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #bnf_ref
            }

            fn fail_at(pos: usize, state: ::tygr::State) -> bool {
                <#source as ::tygr::Grammar>::fail_at(pos, state)
            }
        }

        impl #impl_generics ::tygr::GrammarRule for #self_ty #where_clause {
            const NAME: &'static str = #name;

            fn to_bnf_def() -> ::tygr::bnf::Expr {
                #to_bnf_def
            }
        }
    })
}

fn impl_struct(
    ident: &Ident,
    generics: &Generics,
    name: String,
    hidden: bool,
    inline: bool,
    validated: bool,
    data: &DataStruct,
) -> syn::Result<TokenStream2> {
    let tag = Tag::new(quote! { #ident }, None);
    let fields = &data.fields;
    let FieldsInfo {
        constructor,
        parse_at,
        scan_at,
        print_to,
        to_bnf,
        fail_at,
        first,
    } = FieldsInfo::from(validated, tag, fields);
    let parse_at = with_node(inline, parse_at);
    let scan_at = with_node(inline, scan_at);
    let fail_at = with_node(inline, fail_at);
    let to_bnf = if validated {
        quote! { ::tygr::bnf::Expr::side_condition(#to_bnf, <Self as ::tygr::Validate>::REQUIREMENT) }
    } else {
        to_bnf
    };
    let bnf_ref = bnf_ref(&name, hidden, &to_bnf, inline);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline(always)]
            fn parse_at(input: &[u8], pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_at
            }

            #[inline(always)]
            fn scan_at(input: &[u8], pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_at
            }

            fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                let #constructor = &self;
                #print_to
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #bnf_ref
            }

            fn fail_at(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at
            }
        }

        impl #impl_generics ::tygr::GrammarRule for #ident #ty_generics #where_clause {
            const NAME: &'static str = #name;

            fn to_bnf_def() -> ::tygr::bnf::Expr {
                #to_bnf
            }
        }
    })
}

fn impl_enum(
    ident: &Ident,
    generics: &Generics,
    name: String,
    hidden: bool,
    inline: bool,
    validated: bool,
    data: &DataEnum,
) -> syn::Result<TokenStream2> {
    let mut each_constructor = vec![];
    let mut each_parse_at = vec![];
    let mut each_scan_at = vec![];
    let mut each_print_to = vec![];
    let mut each_to_bnf = vec![];
    let mut each_fail_at = vec![];
    let mut each_first = vec![];
    for variant in &data.variants {
        let variant_ident = &variant.ident;
        let tag = Tag::new(quote! { #ident }, Some(quote! {#variant_ident}));
        let FieldsInfo {
            constructor,
            parse_at,
            scan_at,
            print_to,
            to_bnf,
            fail_at,
            first,
        } = FieldsInfo::from(validated, tag, &variant.fields);
        each_constructor.push(constructor);
        each_parse_at.push(parse_at);
        each_scan_at.push(scan_at);
        each_print_to.push(print_to);
        each_to_bnf.push(to_bnf);
        each_fail_at.push(fail_at);
        each_first.push(first);
    }
    // Every variant reports (`&`, not `&&`); the enum is required only if
    // all variants are.
    let fail_at_variants_body = {
        let mut variants = each_fail_at.iter();
        let first = variants.next().cloned().unwrap_or_else(|| quote! { false });
        variants.fold(first, |acc, next| quote! { (#acc) & (#next) })
    };
    let to_bnf = quote! { ::tygr::bnf::Expr::alternation(vec![ #(#each_to_bnf),* ]) };
    let to_bnf = if validated {
        quote! { ::tygr::bnf::Expr::side_condition(#to_bnf, <Self as ::tygr::Validate>::REQUIREMENT) }
    } else {
        to_bnf
    };
    let bnf_ref = bnf_ref(&name, hidden, &to_bnf, inline);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let n = each_first.len();
    let self_ty = quote! { #ident #ty_generics };
    let parse_fn_ty = quote! {
        fn(&[u8], usize, ::tygr::State<'_>) -> Option<(#self_ty, usize)>
    };
    let scan_fn_ty = quote! {
        fn(&[u8], usize, ::tygr::State<'_>) -> Option<usize>
    };
    let case_fns: Vec<Ident> = (0..n)
        .map(|k| Ident::new(&format!("parse_case_{k}"), Span::call_site()))
        .collect();
    let scan_fns: Vec<Ident> = (0..n)
        .map(|k| Ident::new(&format!("scan_case_{k}"), Span::call_site()))
        .collect();
    let case_defs = case_fns.iter().zip(&each_parse_at).enumerate().map(|(k, (name, arm))| {
        let next = if k + 1 < n {
            let nx = &case_fns[k + 1];
            quote! { Self::#nx(input, pos, state) }
        } else {
            quote! { None }
        };
        quote! {
            #[inline]
            fn #name(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                if let Some(result) = (|| { #arm })() { return Some(result); }
                #next
            }
        }
    });
    let scan_case_defs = scan_fns
        .iter()
        .zip(&each_scan_at)
        .enumerate()
        .map(|(k, (name, arm))| {
            let next = if k + 1 < n {
                let nx = &scan_fns[k + 1];
                quote! { Self::#nx(input, pos, state) }
            } else {
                quote! { None }
            };
            quote! {
                #[inline]
                fn #name(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                    if let Some(result) = (|| { #arm })() { return Some(result); }
                    #next
                }
            }
        });
    // Slots 0-255: next byte; 256: end of input.
    let build_table = |fn_ty: &TokenStream2, fns: &[Ident], miss: &TokenStream2| {
        let indices = 0..fns.len();
        quote! {{
            let parsers: [#fn_ty; #n] = [ #(<#self_ty>::#fns),* ];
            // No variant can start here: `miss` reports them all.
            let mut table: [#fn_ty; 257] = [#miss; 257];
            let mut first = 0usize;
            while first < 257 {
                let next = if first < 256 { Some(first as u8) } else { None };
                table[first] = #( if ::tygr::first::contains::<#each_first>(next) {
                    parsers[#indices]
                } else )* { #miss };
                first += 1;
            }
            table
        }}
    };
    let parse_table = build_table(
        &parse_fn_ty,
        &case_fns,
        &quote! { <#self_ty>::parse_case_miss },
    );
    let scan_table = build_table(
        &scan_fn_ty,
        &scan_fns,
        &quote! { <#self_ty>::scan_case_miss },
    );
    const DISPATCH_THRESHOLD: usize = 4;
    // `cfg!` here: a spliced `#[cfg]` would test the user's crate features.
    let fail_at_on_miss = if cfg!(feature = "trace_pos") {
        quote! { Self::fail_at_variants(pos, state); }
    } else {
        quote! {}
    };
    let (dispatch_body, scan_dispatch_body, case_impls) = if n >= DISPATCH_THRESHOLD {
        (
            quote! {
                const DISPATCH: [#parse_fn_ty; 257] = #parse_table;
                let first = input.get(pos).map(|&first| first as usize).unwrap_or(256);
                DISPATCH[first](input, pos, state)
            },
            quote! {
                const DISPATCH: [#scan_fn_ty; 257] = #scan_table;
                let first = input.get(pos).map(|&first| first as usize).unwrap_or(256);
                DISPATCH[first](input, pos, state)
            },
            quote! {
                #(#case_defs)*
                #(#scan_case_defs)*

                #[inline]
                fn parse_case_miss(_input: &[u8], #[allow(unused_variables)] pos: usize, #[allow(unused_variables)] state: ::tygr::State) -> Option<(Self, usize)> {
                    #fail_at_on_miss
                    None
                }

                #[inline]
                fn scan_case_miss(_input: &[u8], #[allow(unused_variables)] pos: usize, #[allow(unused_variables)] state: ::tygr::State) -> Option<usize> {
                    #fail_at_on_miss
                    None
                }
            },
        )
    } else {
        (
            quote! {
                #( if let Some(result) = (|| { #each_parse_at })() { return Some(result); } )*
                None
            },
            quote! {
                #( if let Some(result) = (|| { #each_scan_at })() { return Some(result); } )*
                None
            },
            quote! {},
        )
    };
    let parse_body = with_node(inline, dispatch_body);
    let scan_body = with_node(inline, scan_dispatch_body);
    let fail_at = with_node(inline, quote! { Self::fail_at_variants(pos, state) });
    let first = {
        let mut the_first = quote! { ::tygr::first::Required<::tygr::first::byte_set::Emp> };
        for first in each_first {
            the_first = quote! { <#the_first as ::tygr::first::First>::Alt<#first> };
        }
        the_first
    };
    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            #case_impls

            #[inline]
            fn parse_case_none(_input: &[u8], _pos: usize, _state: ::tygr::State) -> Option<(Self, usize)> {
                None
            }

            #[inline]
            fn fail_at_variants(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at_variants_body
            }
        }

        impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline(always)]
            fn parse_at(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_body
            }

            #[inline(always)]
            fn scan_at(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_body
            }

            fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                match self {
                    #(#each_constructor => { #each_print_to }),*
                }
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #bnf_ref
            }

            fn fail_at(pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at
            }
        }

        impl #impl_generics ::tygr::GrammarRule for #ident #ty_generics #where_clause {
            const NAME: &'static str = #name;

            fn to_bnf_def() -> ::tygr::bnf::Expr {
                #to_bnf
            }
        }
    })
}
