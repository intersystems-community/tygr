use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_quote, Data, DataEnum, DataStruct, DeriveInput, Fields, Ident};

/// Derive `Grammar` (and `GrammarRule`) for a `struct` or `enum`.
///
/// A `struct` is a sequence of its fields; an `enum` an alternation of its
/// variants. `#[grammar(name = "…", hidden, inline, validated)]` are
/// optional, as is converting a field with
/// `#[grammar(from/try_from/from_str/from_str_infallible = S)]` (see `tygr`'s
/// crate docs).
#[proc_macro_derive(Grammar, attributes(grammar))]
pub fn derive_grammar(input: TokenStream) -> TokenStream {
    derive(input, impl_grammar)
}

/// `Grammar` by parsing `GrammarFrom::Source` and calling
/// [`FromStr`](std::str::FromStr) on the matched text; `Err` backtracks.
/// Takes the same `#[grammar(...)]` options as `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromStr, attributes(grammar))]
pub fn derive_grammar_from_str(input: TokenStream) -> TokenStream {
    derive(input, |input| impl_convert(input, Convert::FromStr))
}

/// `Grammar` by parsing `GrammarFrom::Source` and calling
/// [`FromStr`](std::str::FromStr) on the matched text, whose `Err` must be
/// [`Infallible`](std::convert::Infallible). It can't fail, so scanning skips
/// the conversion. Takes the same `#[grammar(...)]` options as
/// `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromStrInfallible, attributes(grammar))]
pub fn derive_grammar_from_str_infallible(input: TokenStream) -> TokenStream {
    derive(input, |input| {
        impl_convert(input, Convert::FromStrInfallible)
    })
}

/// `Grammar` by parsing `GrammarFrom::Source` and converting with
/// [`From`]. Takes the same `#[grammar(...)]` options as `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarFromOther, attributes(grammar))]
pub fn derive_grammar_from_source(input: TokenStream) -> TokenStream {
    derive(input, |input| impl_convert(input, Convert::From))
}

/// `Grammar` by parsing `GrammarFrom::Source` and converting with
/// [`TryFrom`]; `Err` backtracks. Takes the same `#[grammar(...)]` options as
/// `#[derive(Grammar)]`.
#[proc_macro_derive(GrammarTryFromOther, attributes(grammar))]
pub fn derive_grammar_try_from_source(input: TokenStream) -> TokenStream {
    derive(input, |input| impl_convert(input, Convert::TryFrom))
}

enum Convert {
    FromStr,
    FromStrInfallible,
    From,
    TryFrom,
}

/// Bound every type parameter by `Grammar`, then generate with `expand`.
fn derive(
    input: TokenStream,
    expand: impl FnOnce(&DeriveInput) -> syn::Result<TokenStream2>,
) -> TokenStream {
    let mut input = syn::parse_macro_input!(input as DeriveInput);
    for param in input.generics.type_params_mut() {
        param.bounds.push(parse_quote!(::tygr::Grammar));
    }
    expand(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// `Token<G>`, `G` holding each char's UTF-8 bytes as `ByteEq`s. Not doc
// comments: these would show on `tygr`'s re-exports.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEq(input: TokenStream) -> TokenStream {
    literal(input, "StringEq", char_eq)
}

// Like `StringEq!`, each char an `Either` over its case variants.
#[proc_macro]
#[allow(non_snake_case)]
pub fn StringEqCI(input: TokenStream) -> TokenStream {
    literal(input, "StringEqCI", char_eq_ci)
}

/// `Token<G>`, `G` nesting each char's `char_grammar` as pairs.
fn literal(
    input: TokenStream,
    macro_name: &str,
    char_grammar: fn(char) -> TokenStream2,
) -> TokenStream {
    let lit = syn::parse_macro_input!(input as syn::LitStr);
    let value = lit.value();
    if value.is_empty() {
        let message = format!("{macro_name}!() requires a non-empty literal");
        return syn::Error::new(lit.span(), message)
            .to_compile_error()
            .into();
    }
    let tuple = right_nested(value.chars().map(char_grammar), pair);
    quote! { ::tygr::__private::Token<#tuple> }.into()
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
    let attr = grammar_attr(input)?;
    match &input.data {
        Data::Struct(data) => impl_struct(input, attr, data),
        Data::Enum(data) => impl_enum(input, attr, data),
        Data::Union(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "Grammar cannot be derived for unions",
        )),
    }
}

/// The type's `#[grammar(...)]` options.
struct GrammarAttr {
    name: String,
    hidden: bool,
    inline: bool,
    validated: bool,
}

/// Read all of the type's `#[grammar(...)]` attributes.
fn grammar_attr(input: &DeriveInput) -> syn::Result<GrammarAttr> {
    let mut attr = GrammarAttr {
        name: default_name(&input.ident),
        hidden: false,
        inline: false,
        validated: false,
    };
    for grammar in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("grammar"))
    {
        grammar.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                attr.name = meta.value()?.parse::<syn::LitStr>()?.value();
            } else if meta.path.is_ident("hidden") {
                // Hidden nodes aren't in BNF, so they must be inline too.
                attr.hidden = true;
                attr.inline = true;
            } else if meta.path.is_ident("inline") {
                attr.inline = true;
            } else if meta.path.is_ident("validated") {
                attr.validated = true;
            } else {
                return Err(meta.error("expected `name`, `hidden`, `inline`, or `validated`"));
            }
            Ok(())
        })?;
    }
    Ok(attr)
}

/// A field: its binding `x`, the grammar `source` that parses it (its own
/// type unless converted), and the code that parses, scans, prints and
/// describes it.
struct Field {
    x: Ident,
    source: TokenStream2,
    parse: TokenStream2,
    scan: TokenStream2,
    print: TokenStream2,
    bnf: TokenStream2,
}

/// Record that `start..end` was rejected for `requirement`.
fn rejected(start: &str, end: &str, requirement: TokenStream2) -> TokenStream2 {
    let (start, end) = (format_ident!("{start}"), format_ident!("{end}"));
    if cfg!(feature = "trace") {
        quote! {
            state.expect(#end, ::tygr::Expectation::Rejected {
                node: <Self as ::tygr::GrammarRule>::NAME,
                range: #start..#end,
                requirement: #requirement,
            });
        }
    } else if cfg!(feature = "trace_pos") {
        quote! { state.expect(#end); }
    } else {
        quote! {}
    }
}

/// Reject `value` unless it passes `Validate`, recording `start..end`.
fn validation(start: &str, end: &str) -> TokenStream2 {
    let rejected = rejected(
        start,
        end,
        quote! { <Self as ::tygr::Validate>::REQUIREMENT },
    );
    quote! {
        if !::tygr::Validate::validate(&value) {
            #rejected
            return None;
        }
    }
}

/// `bnf`, footnoted with `Validate::REQUIREMENT` if `validated`.
fn validated_bnf(validated: bool, bnf: TokenStream2) -> TokenStream2 {
    if validated {
        quote! { ::tygr::bnf::Expr::side_condition(#bnf, <Self as ::tygr::Validate>::REQUIREMENT) }
    } else {
        bnf
    }
}

/// `#[grammar(<kind> = S)]`, if present.
fn conversion(attrs: &[syn::Attribute]) -> syn::Result<Option<(String, syn::Type)>> {
    let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("grammar")) else {
        return Ok(None);
    };
    attr.parse_args_with(|input: syn::parse::ParseStream| {
        let kind: Ident = input.parse()?;
        if !["from", "try_from", "from_str", "from_str_infallible"]
            .iter()
            .any(|known| kind == known)
        {
            return Err(syn::Error::new(
                kind.span(),
                "expected `from`, `try_from`, `from_str`, or `from_str_infallible`",
            ));
        }
        let _: syn::Token![=] = input.parse()?;
        Ok(Some((kind.to_string(), input.parse()?)))
    })
}

/// A field, parsed by its own type or converted from a source grammar
/// (`#[grammar(from/try_from/from_str/from_str_infallible = S)]`).
fn field(x: Ident, field: &syn::Field) -> syn::Result<Field> {
    let ty = &field.ty;
    let Some((kind, source)) = conversion(&field.attrs)? else {
        return Ok(Field {
            source: quote! { #ty },
            parse: quote! { let (#x, pos) = <#ty as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?; },
            scan: quote! { let pos = <#ty as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?; },
            print: quote! { ::tygr::Grammar::print_to(#x, buf); },
            bnf: quote! { <#ty as ::tygr::Grammar>::to_bnf() },
            x,
        });
    };
    let helper = format_ident!("{kind}");
    // `pat` is `x` when parsing and `_` when scanning.
    let convert = |pat: TokenStream2| {
        quote! {
            let (#pat, pos) = ::tygr::__private::#helper::<#source, #ty>(input, pos, state.reborrow())?;
        }
    };
    let bnf = quote! { <#source as ::tygr::Grammar>::to_bnf() };
    // A fallible conversion must convert to scan, and is footnoted in BNF.
    let (scan, bnf) = match kind.as_str() {
        "try_from" | "from_str" => (
            convert(quote! { _ }),
            quote! { ::tygr::bnf::Expr::side_condition(#bnf, #CONVERTIBLE) },
        ),
        _ => (
            quote! { let pos = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?; },
            bnf,
        ),
    };
    // A value prints back through its source; text through `Display`.
    let print = match kind.as_str() {
        "from" | "try_from" => quote! {
            ::tygr::Grammar::print_to(&<#source as ::core::convert::From<&#ty>>::from(#x), buf);
        },
        _ => quote! {
            buf.extend_from_slice(::std::string::ToString::to_string(#x).as_bytes());
        },
    };
    Ok(Field {
        parse: convert(quote! { #x }),
        source: quote! { #source },
        scan,
        print,
        bnf,
        x,
    })
}

/// A unit variant's `#[grammar(<any kind> = S)]`: match `S`, keep nothing.
fn discarded(source: syn::Type) -> Field {
    let scan =
        quote! { let pos = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?; };
    Field {
        x: format_ident!("_unit"),
        parse: scan.clone(),
        scan,
        print: quote! {
            ::tygr::Grammar::print_to(&<#source as ::core::default::Default>::default(), buf);
        },
        bnf: quote! { <#source as ::tygr::Grammar>::to_bnf() },
        source: quote! { #source },
    }
}

/// `path`'s constructor, binding its fields as `_f0`, `_f1`, … so no field
/// name clashes with the generated code, and the fields.
fn constructor_and_fields(
    path: TokenStream2,
    fields: &Fields,
    attrs: &[syn::Attribute],
) -> syn::Result<(TokenStream2, Vec<Field>)> {
    if let Fields::Unit = fields {
        let discarded = conversion(attrs)?.map(|(_, source)| discarded(source));
        return Ok((path, discarded.into_iter().collect()));
    }
    if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("grammar")) {
        return Err(syn::Error::new_spanned(
            attr,
            "only a unit variant takes `#[grammar(...)]`; put it on a field",
        ));
    }
    let converted = fields
        .iter()
        .enumerate()
        .map(|(i, syntax)| field(format_ident!("_f{i}"), syntax))
        .collect::<syn::Result<Vec<_>>>()?;
    let xs = converted.iter().map(|field| &field.x);
    let constructor = match fields {
        Fields::Named(named) => {
            let names = named.named.iter().map(|field| &field.ident);
            quote! { #path { #(#names: #xs),* } }
        }
        _ => quote! { #path ( #(#xs),* ) },
    };
    Ok((constructor, converted))
}

/// Parse each field, then build (and maybe validate) the value.
fn parse_at(validated: bool, fields: &[Field], constructor: &TokenStream2) -> TokenStream2 {
    let steps = fields.iter().map(|field| &field.parse);
    let (start_pos, validation) = if validated {
        (
            quote! { let start_pos = pos; },
            validation("start_pos", "pos"),
        )
    } else {
        (quote! {}, quote! {})
    };
    quote! {
        #start_pos
        #(#steps)*
        let value = #constructor;
        #validation
        Some((value, pos))
    }
}

/// Scan each field; validating needs the value, so that parses instead.
fn scan_at(validated: bool, fields: &[Field], constructor: &TokenStream2) -> TokenStream2 {
    if validated {
        let parse_at = parse_at(true, fields, constructor);
        return quote! { ({ #parse_at }).map(|(_, pos)| pos) };
    }
    let steps = fields.iter().map(|field| &field.scan);
    quote! { #(#steps)* Some(pos) }
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

/// The code for a struct's fields, or one variant's.
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
    fn new(
        validated: bool,
        path: TokenStream2,
        fields: &Fields,
        attrs: &[syn::Attribute],
    ) -> syn::Result<Self> {
        let (constructor, fields) = constructor_and_fields(path, fields, attrs)?;
        let prints = fields.iter().map(|field| &field.print);
        let bnfs = fields.iter().map(|field| &field.bnf);
        let sources: Vec<_> = fields.iter().map(|field| &field.source).collect();
        Ok(Self {
            parse_at: parse_at(validated, &fields, &constructor),
            scan_at: scan_at(validated, &fields, &constructor),
            print_to: quote! { #(#prints)* },
            to_bnf: quote! { ::tygr::bnf::Expr::sequence(vec![#(#bnfs),*]) },
            // Stops after the first required field.
            fail_at: quote! {
                #(<#sources as ::tygr::Grammar>::fail_at(pos, state.reborrow()) ||)* false
            },
            first: sources.iter().fold(
                quote! { ::tygr::first::Optional<::tygr::first::byte_set::Emp> },
                |first, source| quote! { <#first as ::tygr::first::First>::Seq<#source> },
            ),
            constructor,
        })
    }
}

/// `print_fn` with the `print` feature. Otherwise a `print_to` that runs
/// `read_fields` (so the fields count as read) and panics.
fn printing(print_fn: TokenStream2, read_fields: TokenStream2) -> TokenStream2 {
    if cfg!(feature = "print") {
        return print_fn;
    }
    quote! {
        fn print_to(&self, _buf: &mut ::std::vec::Vec<u8>) {
            #read_fields
            unimplemented!("tygr's `print` feature is off")
        }
    }
}

/// The requirement a `GrammarFrom*` conversion adds, in BNF and errors.
const CONVERTIBLE: &str = "be convertible";

/// The bodies of a derived `Grammar` and `GrammarRule`.
struct Impls {
    name: String,
    first: TokenStream2,
    parse_at: TokenStream2,
    scan_at: TokenStream2,
    /// The whole `print_to` fn, from [`printing`].
    print_to: TokenStream2,
    to_bnf: TokenStream2,
    fail_at: TokenStream2,
    to_bnf_def: TokenStream2,
}

impl Impls {
    fn emit(self, input: &DeriveInput) -> TokenStream2 {
        let Impls {
            name,
            first,
            parse_at,
            scan_at,
            print_to,
            to_bnf,
            fail_at,
            to_bnf_def,
        } = self;
        let ident = &input.ident;
        let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
        quote! {
            impl #impl_generics ::tygr::Grammar for #ident #ty_generics #where_clause {
                type First = #first;

                #[inline(always)]
                fn parse_at(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<(Self, usize)> {
                    #parse_at
                }

                #[inline(always)]
                fn scan_at(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<usize> {
                    #scan_at
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
                    #to_bnf_def
                }
            }
        }
    }
}

fn impl_convert(input: &DeriveInput, convert: Convert) -> syn::Result<TokenStream2> {
    let GrammarAttr {
        name,
        hidden,
        inline,
        validated,
    } = grammar_attr(input)?;
    let ident = &input.ident;
    let (_, ty_generics, _) = input.generics.split_for_impl();
    let self_ty = quote! { #ident #ty_generics };
    let source = quote! { <#self_ty as ::tygr::GrammarFrom>::Source };
    let text = quote! {
        let end = <#source as ::tygr::Grammar>::scan_at(input, pos, state.reborrow())?;
        // SAFETY: see the invariant on `Grammar::parse_at`.
        let text = unsafe { ::core::str::from_utf8_unchecked(&input[pos..end]) };
    };
    let parsed = quote! {
        let (source, end) = <#source as ::tygr::Grammar>::parse_at(input, pos, state.reborrow())?;
    };
    let reject = rejected("pos", "end", quote! { #CONVERTIBLE });
    // Bind `value` and `end`, or reject.
    let (converted, fallible) = match convert {
        Convert::FromStr => (
            quote! {
                #text
                let Ok(value) = <#self_ty as ::core::str::FromStr>::from_str(text) else {
                    #reject
                    return None;
                };
            },
            true,
        ),
        Convert::FromStrInfallible => (
            quote! {
                #text
                let result: ::core::result::Result<#self_ty, ::core::convert::Infallible> =
                    <#self_ty as ::core::str::FromStr>::from_str(text);
                let Ok(value) = result;
            },
            false,
        ),
        Convert::From => (
            quote! {
                #parsed
                let value = <#self_ty as ::core::convert::From<#source>>::from(source);
            },
            false,
        ),
        Convert::TryFrom => (
            quote! {
                #parsed
                let Ok(value) = <#self_ty as ::core::convert::TryFrom<#source>>::try_from(source) else {
                    #reject
                    return None;
                };
            },
            true,
        ),
    };
    let validation = if validated {
        validation("pos", "end")
    } else {
        quote! {}
    };
    // Only infallible, unvalidated conversions can scan without converting.
    let scan_at = if fallible || validated {
        quote! { Self::parse_at(input, pos, state).map(|(_, end)| end) }
    } else {
        quote! { <#source as ::tygr::Grammar>::scan_at(input, pos, state) }
    };
    let to_bnf = quote! { <#source as ::tygr::Grammar>::to_bnf() };
    // Side-conditions only on the rule's own definition, not references.
    let to_bnf_def = if fallible {
        quote! { ::tygr::bnf::Expr::side_condition(#to_bnf, #CONVERTIBLE) }
    } else {
        to_bnf.clone()
    };
    Ok(Impls {
        to_bnf: bnf_ref(&name, hidden, &to_bnf, inline),
        to_bnf_def: validated_bnf(validated, to_bnf_def),
        name,
        first: quote! { <#source as ::tygr::Grammar>::First },
        parse_at: quote! {
            #converted
            #validation
            Some((value, end))
        },
        scan_at,
        print_to: printing(
            quote! {
                fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                    <#self_ty as ::tygr::GrammarFrom>::print_to(self, buf);
                }
            },
            quote! {},
        ),
        fail_at: quote! { <#source as ::tygr::Grammar>::fail_at(pos, state) },
    }
    .emit(input))
}

fn impl_struct(
    input: &DeriveInput,
    attr: GrammarAttr,
    data: &DataStruct,
) -> syn::Result<TokenStream2> {
    let GrammarAttr {
        name,
        hidden,
        inline,
        validated,
    } = attr;
    let ident = &input.ident;
    let FieldsInfo {
        constructor,
        parse_at,
        scan_at,
        print_to,
        to_bnf,
        fail_at,
        first,
    } = FieldsInfo::new(validated, quote! { #ident }, &data.fields, &[])?;
    let to_bnf_def = validated_bnf(validated, to_bnf);
    Ok(Impls {
        to_bnf: bnf_ref(&name, hidden, &to_bnf_def, inline),
        to_bnf_def,
        name,
        first,
        parse_at: with_node(inline, parse_at),
        scan_at: with_node(inline, scan_at),
        print_to: printing(
            quote! {
                fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                    let #constructor = self;
                    #print_to
                }
            },
            quote! { let #constructor = self; },
        ),
        fail_at: with_node(inline, fail_at),
    }
    .emit(input))
}

/// The variant count from which an enum dispatches on a table.
const DISPATCH_THRESHOLD: usize = 4;

/// The `parse_at` or `scan_at` body trying `arms` (one per variant, ending
/// in `Some(output)`) in order, and the inherent items it needs. With enough
/// variants, a table on the next byte skips to the first that can start
/// there.
fn dispatch(
    kind: &str,
    output: TokenStream2,
    arms: &[&TokenStream2],
    firsts: &[&TokenStream2],
) -> (TokenStream2, TokenStream2) {
    let n = arms.len();
    if n < DISPATCH_THRESHOLD {
        let body = quote! {
            #( if let Some(result) = (|| { #arms })() { return Some(result); } )*
            None
        };
        return (body, quote! {});
    }
    let cases: Vec<Ident> = (0..n).map(|k| format_ident!("{kind}_case_{k}")).collect();
    let nexts = cases
        .iter()
        .skip(1)
        .map(|next| quote! { Self::#next(input, pos, state) })
        .chain([quote! { None }]);
    let miss = format_ident!("{kind}_miss");
    let table = format_ident!("{}_TABLE", kind.to_uppercase());
    let fn_ty = quote! { fn(&[u8], usize, ::tygr::State<'_>) -> Option<#output> };
    // `cfg!` here: a spliced `#[cfg]` would test the user's crate features.
    let report = if cfg!(feature = "trace_pos") {
        quote! { Self::fail_at_variants(pos, state); }
    } else {
        quote! {}
    };
    let indices = 0..n;
    let items = quote! {
        #(
            #[inline]
            fn #cases(input: &[u8], pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> Option<#output> {
                if let Some(result) = (|| { #arms })() { return Some(result); }
                #nexts
            }
        )*

        // No variant can start here: report them all.
        #[inline]
        fn #miss(_input: &[u8], #[allow(unused_variables)] pos: usize, #[allow(unused_variables)] state: ::tygr::State) -> Option<#output> {
            #report
            None
        }

        // Slots 0-255: the next byte; 256: end of input.
        const #table: [#fn_ty; 257] = {
            let cases: [#fn_ty; #n] = [#(Self::#cases),*];
            let mut table: [#fn_ty; 257] = [Self::#miss; 257];
            let mut slot = 0;
            while slot < 257 {
                let next = if slot < 256 { Some(slot as u8) } else { None };
                table[slot] = #( if ::tygr::first::contains::<#firsts>(next) {
                    cases[#indices]
                } else )* { Self::#miss };
                slot += 1;
            }
            table
        };
    };
    let body = quote! {
        Self::#table[input.get(pos).map_or(256, |&byte| byte as usize)](input, pos, state)
    };
    (body, items)
}

fn impl_enum(input: &DeriveInput, attr: GrammarAttr, data: &DataEnum) -> syn::Result<TokenStream2> {
    let GrammarAttr {
        name,
        hidden,
        inline,
        validated,
    } = attr;
    let ident = &input.ident;
    let variants = data
        .variants
        .iter()
        .map(|variant| {
            let path = &variant.ident;
            let path = quote! { #ident::#path };
            FieldsInfo::new(validated, path, &variant.fields, &variant.attrs)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let constructors: Vec<_> = variants
        .iter()
        .map(|variant| &variant.constructor)
        .collect();
    let parse_arms: Vec<_> = variants.iter().map(|variant| &variant.parse_at).collect();
    let scan_arms: Vec<_> = variants.iter().map(|variant| &variant.scan_at).collect();
    let prints = variants.iter().map(|variant| &variant.print_to);
    let bnfs = variants.iter().map(|variant| &variant.to_bnf);
    let fail_ats = variants.iter().map(|variant| &variant.fail_at);
    let firsts: Vec<_> = variants.iter().map(|variant| &variant.first).collect();
    let (parse_at, parse_items) = dispatch("parse", quote! { (Self, usize) }, &parse_arms, &firsts);
    let (scan_at, scan_items) = dispatch("scan", quote! { usize }, &scan_arms, &firsts);
    let to_bnf_def = validated_bnf(
        validated,
        quote! { ::tygr::bnf::Expr::alternation(vec![#(#bnfs),*]) },
    );
    let impls = Impls {
        to_bnf: bnf_ref(&name, hidden, &to_bnf_def, inline),
        to_bnf_def,
        name,
        first: firsts.iter().fold(
            quote! { ::tygr::first::Required<::tygr::first::byte_set::Emp> },
            |first, variant| quote! { <#first as ::tygr::first::First>::Alt<#variant> },
        ),
        parse_at: with_node(inline, parse_at),
        scan_at: with_node(inline, scan_at),
        print_to: printing(
            quote! {
                fn print_to(&self, buf: &mut ::std::vec::Vec<u8>) {
                    match self {
                        #(#constructors => { #prints })*
                    }
                }
            },
            quote! { match self { #(#constructors => {})* } },
        ),
        fail_at: with_node(inline, quote! { Self::fail_at_variants(pos, state) }),
    }
    .emit(input);
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            #parse_items
            #scan_items

            // Every variant reports (`&`, not `&&`); the enum is required only
            // if all variants are.
            #[inline]
            fn fail_at_variants(#[allow(unused_variables)] pos: usize, #[allow(unused_variables, unused_mut)] mut state: ::tygr::State) -> bool {
                #((#fail_ats) &)* true
            }
        }

        #impls
    })
}
