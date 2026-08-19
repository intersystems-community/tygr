use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{parse_quote, Data, DataEnum, DataStruct, DeriveInput, Fields, Generics, Ident};

/// Derive `Grammar` (and `GrammarRule`) for a `struct` or `enum`.
///
/// `struct`s become a concatenation of their fields; `enum`s become an
/// alternation of their variants' own fields — see `tygr`'s crate-level
/// `Design` section for the full mapping from Rust constructs to EBNF.
///
/// `#[grammar(...)]` on the derived type is optional:
/// - `name = "..."` — override the BNF rule name (defaults to the type name).
/// - `hidden` — omit this type from BNF output; parsing/printing unaffected.
/// - `inline` — splice this type's own definition wherever it's
///   referenced, instead of a rule reference.
/// - `validated` — after a successful parse, run `Validate::validate` on the
///   value; a rejection backtracks as if the grammar hadn't matched.
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

/// `Grammar` via `GrammarFrom` + [`FromStr`](std::str::FromStr) on the matched text.
///
/// Parses `GrammarFrom::Source`, then calls `Self::from_str` on the exact
/// substring it matched. `Err` backtracks the parse as if nothing had
/// matched, and is traced under the `trace`/`trace_pos` features.
#[proc_macro_derive(GrammarFromStr)]
pub fn derive_grammar_from_str(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::FromStr)
}

/// `Grammar` via `GrammarFrom` + [`From<Source>`](From).
///
/// Parses `GrammarFrom::Source`, then builds `Self` with `From::from` — this
/// conversion can't fail, so nothing is traced.
#[proc_macro_derive(GrammarFromOther)]
pub fn derive_grammar_from_source(input: TokenStream) -> TokenStream {
    derive_convert(input, Convert::From)
}

/// `Grammar` via `GrammarFrom` + [`TryFrom<Source>`](TryFrom).
///
/// Parses `GrammarFrom::Source`, then calls `Self::try_from`. `Err`
/// backtracks the parse as if nothing had matched, and is traced under the
/// `trace`/`trace_pos` features.
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
/// This is the *primary way* to create literal tokens — the underlying
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

fn with_node(inline: bool, body: TokenStream2) -> TokenStream2 {
    if inline {
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
        inline,
        validated,
    } = grammar_attr(input)?;
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
            impl_struct(ident, generics, name, hidden, inline, validated, data)
        }
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
                // Hidden nodes are not presented in BNF so they must be inline
                // as non-inline node names are presented in traces
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
                state.expect(pos, ::tygr::Expectation::Valid { pos: start_pos, be_valid });
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
                if let Some(be_valid) = ::tygr::Validation::be_valid(::tygr::Validate::validate(&value)) {
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

/// `A::fail_at(..) || B::fail_at(..) || .. || false`: each field reports itself
/// unconditionally; a required (non-nullable) field short-circuits the rest,
/// since real parsing would never reach them either.
fn fail_at(fields: &ProcessedFields) -> TokenStream2 {
    let calls = fields.iter().map(|(_, _, t)| {
        quote! { <#t as ::tygr::Grammar>::fail_at(pos, state.reborrow()) }
    });
    quote! {
        #(#calls ||)* false
    }
}

fn bnf_ref(
    grammar_name: &str,
    hidden: bool,
    to_bnf: TokenStream2,
    inline: bool,
) -> TokenStream2 {
    if hidden {
        quote! { ::tygr::bnf::Expr::empty() }
    } else if inline {
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
    fail_at: TokenStream2,
    first: TokenStream2,
}

impl FieldsInfo {
    fn from(validated: bool, tag: Tag, fields: &Fields) -> Self {
        let (constructor, fields) = components_and_xts(tag.as_constructor(), fields);
        let print_steps = print_steps(&fields);
        let parse_at = parse_at(validated, &fields, &constructor);
        let scan_at = scan_at(validated, &fields, &constructor);
        let mut first = quote! { ::tygr::OptionalFirst<::tygr::EmptyByteSet> };
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
            fail_at: fail_at(&fields),
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
                end,
                ::tygr::Expectation::GrammarFrom(::std::string::ToString::to_string(&err)),
            );
        }
    } else if cfg!(feature = "trace_pos") {
        quote! {
            state.expect(end);
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

            fn fail_at(pos: usize, state: ::tygr::State) -> bool {
                <#source as ::tygr::Grammar>::fail_at(pos, state)
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
    let bnf_ref = bnf_ref(&name, hidden, to_bnf.clone(), inline);
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

// ── Enum → alternation ──────────────────────────────────────────────────────

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
    // Alternatives aren't sequential: every variant reports itself unconditionally
    // (no short-circuiting between them). The enum as a whole is required only if
    // every variant is (`&`, not `&&`, so all still get called for their side effects).
    let fail_at_variants_body = {
        let mut variants = each_fail_at.iter();
        let first = variants
            .next()
            .cloned()
            .unwrap_or_else(|| quote! { false });
        variants.fold(first, |acc, next| quote! { (#acc) & (#next) })
    };
    let to_bnf = quote! { ::tygr::bnf::Expr::alternation(vec![ #(#each_to_bnf),* ]) };
    let bnf_ref = bnf_ref(&name, hidden, to_bnf.clone(), inline);
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
    let build_table = |fn_ty: &TokenStream2, fns: &[Ident], miss: &TokenStream2| {
        quote! {{
            let parsers: [#fn_ty; #n] = [ #(<#self_ty>::#fns),* ];
            // No case's FIRST set contains this byte, so none can match: dispatch
            // straight to `miss`, which reports every variant's expectation in
            // O(1) without attempting any of their (guaranteed-failing) real parses.
            let mut table: [#fn_ty; 257] = [#miss; 257];
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
    let parse_table = build_table(&parse_fn_ty, &case_fns, &quote! { <#self_ty>::parse_case_miss });
    let scan_table = build_table(&scan_fn_ty, &scan_fns, &quote! { <#self_ty>::scan_case_miss });
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

                #[inline]
                fn parse_case_miss(_input: &str, #[allow(unused_variables)] pos: usize, #[allow(unused_variables)] state: ::tygr::State) -> Option<(Self, usize)> {
                    #[cfg(feature = "trace_pos")]
                    Self::fail_at_variants(pos, state);
                    None
                }

                #[inline]
                fn scan_case_miss(_input: &str, #[allow(unused_variables)] pos: usize, #[allow(unused_variables)] state: ::tygr::State) -> Option<usize> {
                    #[cfg(feature = "trace_pos")]
                    Self::fail_at_variants(pos, state);
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
    let fail_at = with_node(
        inline,
        quote! { Self::fail_at_variants(pos, state) },
    );
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

            #[inline]
            fn fail_at_variants(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #fail_at_variants_body
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
