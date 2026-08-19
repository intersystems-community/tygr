use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, Data, DataEnum, DataStruct, DeriveInput, Fields, Generics, Ident};

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

/// `Grammar` via `GrammarFrom` + `FromStr` on the matched text (`Err` is traced).
#[proc_macro_derive(GrammarFromStr)]
pub fn derive_grammar_from_str(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::FromStr)
}

/// `Grammar` via `GrammarFrom` + `From<Source>`.
#[proc_macro_derive(GrammarFromOther)]
pub fn derive_grammar_from_source(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::From)
}

/// `Grammar` via `GrammarFrom` + `TryFrom<Source>` (`Error` is traced on rejection).
#[proc_macro_derive(GrammarTryFromOther)]
pub fn derive_grammar_try_from_source(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::TryFrom)
}

#[derive(Clone, Copy)]
enum Convert {
    FromStr,
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

/// Expands a string literal into a type-level literal token.
///
/// This is the **primary way** to create literal tokens — the underlying
/// `StringEq<CharThen<CH, T>>` type is an implementation detail and should not
/// be named directly.
///
/// - Single character: `StringEq!(",")` → `StringEq<CharThen<',', ()>>`
/// - Multiple characters: `StringEq!("->")` → `StringEq<CharThen<'-', CharThen<'>', ()>>>`
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEq(input: TokenStream) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    let chars: Vec<char> = value.chars().collect();

    if chars.is_empty() {
        return syn::Error::new(
            lit.span(),
            "StringEq!() requires a non-empty string literal",
        )
        .to_compile_error()
        .into();
    }

    let chain = build_nested_chain(&chars, |ch, rest| quote! { ::tygr::CharThen<#ch, #rest> });
    quote! { ::tygr::StringEq<#chain> }.into()
}

/// Build a right-nested chain of literal-token types terminated by `()`.
fn build_nested_chain<F>(items: &[char], mapper: F) -> TokenStream2
where
    F: Fn(&char, TokenStream2) -> TokenStream2,
{
    items
        .iter()
        .rev()
        .fold(quote! { () }, |rest, ch| mapper(ch, rest))
}

#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEqCI(input: TokenStream) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    let chars: Vec<char> = value.chars().collect();

    if chars.is_empty() {
        return syn::Error::new(
            lit.span(),
            "StringEqCI!() requires a non-empty string literal",
        )
        .to_compile_error()
        .into();
    }

    let chain = build_nested_chain(&chars, |ch, rest| quote! { ::tygr::CharCIThen<#ch, #rest> });
    quote! { ::tygr::StringEqCI<#chain> }.into()
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

fn with_node(transparent: bool, body: TokenStream2) -> TokenStream2 {
    if transparent {
        body
    } else {
        quote! {
            let mut state = state.node(Self::NAME, pos);
            #body
        }
    }
}

fn impl_grammar(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let ident = &input.ident;
    let generics = &input.generics;
    let GrammarAttr {
        name,
        hidden,
        transparent,
        filtered,
    } = grammar_attr(input);
    let name = name.unwrap_or_else(|| {
        let ident = ident.to_string();
        #[cfg(feature = "lower_bnf_name")]
        let ident = ident.to_ascii_lowercase();
        #[cfg(feature = "upper_bnf_name")]
        let ident = ident.to_ascii_uppercase();
        ident
    });
    match &input.data {
        Data::Struct(data) => {
            impl_struct(ident, generics, name, hidden, transparent, filtered, data)
        }
        Data::Enum(data) => impl_enum(ident, generics, name, hidden, transparent, filtered, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            ident,
            "Grammar cannot be derived for unions",
        )),
    }
}

fn grammar_attr(input: &DeriveInput) -> GrammarAttr {
    input
        .attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("grammar") {
                attr.parse_args::<GrammarAttr>().ok()
            } else {
                None
            }
        })
        .next()
        .unwrap_or_default()
}

#[derive(Default)]
struct GrammarAttr {
    name: Option<String>,
    hidden: bool,
    transparent: bool,
    filtered: bool,
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
                // Hidden nodes are not presented in BNF so they must be transparent
                // as non-transparent node names are presented in traces
                attr.transparent = true;
            } else if ident == "transparent" {
                attr.transparent = true;
            } else if ident == "filtered" {
                attr.filtered = true;
            } else {
                return Err(syn::Error::new(
                    ident.span(),
                    "expected `name`, `hidden`, `transparent`, or `filtered`",
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

fn parse_at(filtered: bool, fields: &ProcessedFields, constructor: &TokenStream2) -> TokenStream2 {
    let steps: Vec<_> = fields
        .iter()
        .map(|(_, x, t)| {
            quote! {
                let (#x, pos) = <#t as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            }
        })
        .collect();
    let filter = if filtered {
        quote! {
            if let Some(be_valid) = ::tygr::FilterResult::be_valid(value.filter()) {
                return None
            }
        }
    } else {
        quote! {}
    };
    quote! {
        #(#steps)*
        let value = #constructor;
        #filter
        Some((value, pos))
    }
}

fn scan_at(filtered: bool, fields: &ProcessedFields, constructor: &TokenStream2) -> TokenStream2 {
    if filtered {
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

fn bnf_ref(
    grammar_name: &str,
    hidden: bool,
    to_bnf: TokenStream2,
    transparent: bool,
) -> TokenStream2 {
    if hidden {
        quote! { ::tygr::bnf::Expr::Empty }
    } else if transparent {
        to_bnf
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
    first: TokenStream2,
}

impl FieldsInfo {
    fn from(filtered: bool, tag: Tag, fields: &Fields) -> Self {
        let (constructor, fields) = components_and_xts(tag.as_constructor(), fields);
        let print_steps = print_steps(&fields);
        let parse_at = parse_at(filtered, &fields, &constructor);
        let scan_at = scan_at(filtered, &fields, &constructor);
        let mut first = quote! { ::tygr::EmptyFirst };
        for (_, _, t) in &fields {
            first = quote! {
                <#first as ::tygr::First>::Concat<#t>
            }
        }
        Self {
            parse_at: quote! { #parse_at },
            scan_at: quote! { #scan_at },
            print_to: quote! {#(#print_steps)*},
            to_bnf: to_bnf(&fields),
            constructor,
            first,
        }
    }
}

fn impl_convert(input: &DeriveInput, convert: Convert) -> syn::Result<TokenStream2> {
    let ident = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let self_ty = quote! { #ident #ty_generics };
    let source = quote! { <#self_ty as ::tygr::GrammarFrom>::Source };
    // parse_at: parse Source, then build Self via the forward conversion.
    let trace = if cfg!(feature = "trace") {
        quote! {
            state.expect(
                pos,
                ::tygr::Expectation::Conversion(::std::string::ToString::to_string(&err)),
            );
        }
    } else if cfg!(feature = "trace_pos") {
        quote! {
            state.expect(pos);
        }
    } else {
        quote! {}
    };
    let parse_at = match convert {
        Convert::FromStr => quote! {
            let end = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?;
            match <#self_ty as ::core::str::FromStr>::from_str(&input[pos..end]) {
                Ok(value) => Some((value, end)),
                Err(err) => {
                    #trace
                    let _ = &err;
                    None
                }
            }
        },
        Convert::From => quote! {
            let (source, end) = <#source as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            Some((<#self_ty as ::core::convert::From<#source>>::from(source), end))
        },
        Convert::TryFrom => quote! {
            let (source, end) = <#source as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
            match <#self_ty as ::core::convert::TryFrom<#source>>::try_from(source) {
                Ok(value) => Some((value, end)),
                Err(err) => {
                    #trace
                    let _ = &err;
                    None
                }
            }
        },
    };
    // scan_at: `From` can't fail beyond Source; `FromStr`/`TryFrom` run the check.
    let scan_at = match convert {
        Convert::From => quote! {
            <#source as ::tygr::Grammar>::scan_at(input, pos, state)
        },
        Convert::FromStr | Convert::TryFrom => quote! {
            Self::parse_at(input, pos, state).map(|(_, end)| end)
        },
    };
    Ok(quote! {
        impl #impl_generics ::tygr::Grammar for #self_ty #where_clause {
            type First = <#source as ::tygr::Grammar>::First;

            #[inline]
            fn parse_at(input: &str, pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_at
            }

            #[inline]
            fn scan_at(input: &str, pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_at
            }

            fn print_to(&self, buf: &mut ::std::string::String) {
                <#self_ty as ::tygr::GrammarFrom>::print_to(self, buf);
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                <#source as ::tygr::Grammar>::to_bnf()
            }
        }
    })
}

fn impl_struct(
    ident: &Ident,
    generics: &Generics,
    name: String,
    hidden: bool,
    transparent: bool,
    filtered: bool,
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
        first,
    } = FieldsInfo::from(filtered, tag, fields);
    let parse_at = with_node(transparent, parse_at);
    let scan_at = with_node(transparent, scan_at);
    let bnf_ref = bnf_ref(&name, hidden, to_bnf.clone(), transparent);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline]
            fn parse_at(input: &str, pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_at
            }

            #[inline]
            fn scan_at(input: &str, pos: usize, #[allow(unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_at
            }

            fn print_to(&self, buf: &mut ::std::string::String) {
                let #constructor = &self;
                #print_to
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #bnf_ref
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

// ── Enum → alternation ──────────────────────────────────────────────────────

fn impl_enum(
    ident: &Ident,
    generics: &Generics,
    name: String,
    hidden: bool,
    transparent: bool,
    filtered: bool,
    data: &DataEnum,
) -> syn::Result<TokenStream2> {
    let mut each_constructor = vec![];
    let mut each_parse_at = vec![];
    let mut each_scan_at = vec![];
    let mut each_print_to = vec![];
    let mut each_to_bnf = vec![];
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
            first,
        } = FieldsInfo::from(filtered, tag, &variant.fields);
        each_constructor.push(constructor);
        each_parse_at.push(parse_at);
        each_scan_at.push(scan_at);
        each_print_to.push(print_to);
        each_to_bnf.push(to_bnf);
        each_first.push(first);
    }
    let to_bnf = quote! { ::tygr::bnf::Expr::alternation(vec![ #(#each_to_bnf),* ]) };
    let bnf_ref = bnf_ref(&name, hidden, to_bnf.clone(), transparent);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let n = each_first.len();
    let self_ty = quote! { #ident #ty_generics };
    let parse_fn_ty = quote! {
        fn(&str, usize, ::tygr::State<'_>) -> Option<(#self_ty, usize)>
    };
    let scan_fn_ty = quote! {
        fn(&str, usize, ::tygr::State<'_>) -> Option<usize>
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
            fn #name(input: &str, pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
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
                fn #name(input: &str, pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                    if let Some(result) = (|| { #arm })() { return Some(result); }
                    #next
                }
            }
        });
    // slots 0-255: u8 / byte / ascii char
    // slots 256  : EOF
    let build_table = |fn_ty: &TokenStream2, fns: &[Ident]| {
        quote! {{
            let parsers: [#fn_ty; #n] = [ #(<#self_ty>::#fns),* ];
            let mut table: [#fn_ty; 257] = [parsers[#n - 1]; 257];
            let mut first = 0usize;
            while first < 257 {
                let mut case = 0usize;
                while case < #n {
                    if CONTAINS_NIL[case] || (first <= 255 && CONTAINS_BYTE[case][first]) {
                        table[first] = parsers[case];
                        break;
                    }
                    case += 1;
                }
                first += 1;
            }
            table
        }}
    };
    let parse_table = build_table(&parse_fn_ty, &case_fns);
    let scan_table = build_table(&scan_fn_ty, &scan_fns);
    const DISPATCH_THRESHOLD: usize = 4;
    let (dispatch_body, scan_dispatch_body, case_impls) = if n >= DISPATCH_THRESHOLD {
        (
            quote! {
                const CONTAINS_NIL: [bool; #n] = [ #(<#each_first as ::tygr::First>::CONTAINS_NIL),* ];
                const CONTAINS_BYTE: [[bool; 256]; #n] = [ #(<#each_first as ::tygr::First>::CONTAINS_BYTE),* ];
                const DISPATCH: [#parse_fn_ty; 257] = #parse_table;
                let first = input.as_bytes().get(pos).map(|&first| first as usize).unwrap_or(256);
                DISPATCH[first](input, pos, state)
            },
            quote! {
                const CONTAINS_NIL: [bool; #n] = [ #(<#each_first as ::tygr::First>::CONTAINS_NIL),* ];
                const CONTAINS_BYTE: [[bool; 256]; #n] = [ #(<#each_first as ::tygr::First>::CONTAINS_BYTE),* ];
                const DISPATCH: [#scan_fn_ty; 257] = #scan_table;
                let first = input.as_bytes().get(pos).map(|&first| first as usize).unwrap_or(256);
                DISPATCH[first](input, pos, state)
            },
            quote! {
                #(#case_defs)*
                #(#scan_case_defs)*
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
    let parse_body = with_node(transparent, dispatch_body);
    let scan_body = with_node(transparent, scan_dispatch_body);
    let first = {
        let mut the_first = quote! { ::tygr::EmptyByteSet };
        for first in each_first {
            the_first = quote! { <#the_first as ::tygr::First>::Union<#first> };
        }
        the_first
    };
    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            #case_impls

            #[inline]
            fn parse_case_none(_input: &str, _pos: usize, _state: ::tygr::State) -> Option<(Self, usize)> {
                None
            }
        }

        impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
            type First = #first;

            #[inline]
            fn parse_at(input: &str, pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                #parse_body
            }

            #[inline]
            fn scan_at(input: &str, pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                #scan_body
            }

            fn print_to(&self, buf: &mut ::std::string::String) {
                match self {
                    #(#each_constructor => { #each_print_to }),*
                }
            }

            fn to_bnf() -> ::tygr::bnf::Expr {
                #bnf_ref
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
