# 🐯 tygr: TYpes into Grammar Routines

Define your grammar once as Rust types and get **parser**, **printer**, and **presentation in EBNF** for free.

## Example

Given this EBNF:

```
Expr  = Expr1 { Op1 Expr1 } ;
Op1   = "+" | "-" ;
Expr1 = Expr2 { Op2 Expr2 } ;
Op2   = "*" | "/" ;
Expr2 = "(" Expr ")" | Int ;
Int   = 'digit' { 'digit' } ;
```

Write it once in Rust:

```rust
use tygr::*;

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Expr(pub Expr1, pub Vec<(Wrap<Ws, Op1, Ws>, Expr1)>);

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Op1 {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Expr1(pub Expr2, pub Vec<(Wrap<Ws, Op2, Ws>, Expr2)>);

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Op2 {
    Mul(StringEq!("*")),
    Div(StringEq!("/")),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Expr2 {
    Paren(Wrap<(StringEq!("("), Ws), Box<Expr>, (Ws, StringEq!(")"))>),
    Number(Int),
}

char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Int(pub StringOf1<IsDigit>); // StringOf1: one or more digits

char_class!(pub IsSpace, "space", |ch| ch.is_ascii_whitespace());

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(hidden)] // parsed & printed, but omitted from the BNF
pub struct Ws(pub StringOf<IsSpace>); // StringOf: zero or more spaces
```

Get parser, printer, and EBNF generator:

```rust
let e = Expr::parse("1 + 2 * 3").unwrap();  // parse
assert_eq!(e.print(), "1 + 2 * 3");         // round-trip print
assert_eq!(bnf_rules![Expr, Op1, Expr1, Op2, Expr2, Int].to_string(),
            "\
Expr = Expr1 { Op1 Expr1 } ;
Op1 = \"+\" | \"-\" ;
Expr1 = Expr2 { Op2 Expr2 } ;
Op2 = \"*\" | \"/\" ;
Expr2 = \"(\" Expr \")\" | Int ;
Int = 'digit' { 'digit' } ;"
        );
```

## More examples

- [`arith`](examples/arith.rs) — the grammar above, as a runnable example.
- [`json`](examples/json.rs) — a full JSON grammar.
- [`json_optimized`](examples/json_optimized.rs) — the same JSON grammar, hand-tuned for parsing throughput.

Each is a small CLI:

```sh
cargo run --example json -- bnf              # print the grammar as EBNF
echo '{"a": [1, true]}' | cargo run --example json -- parse   # parse, print the AST
echo '{"a": 1}'         | cargo run --example json -- test    # check well-formed (exit 0/1)
```

The optimized JSON grammar is also benchmarked (`cargo bench`) against the inputs
in [`data/`](data/).

## Feature flags

The `tygr` crate enables the `trace_one_node` feature by default. Available features are:

| Feature | Description |
| ------- | ----------- |
| `default` | Enables `trace_one_node`. |
| `trace` | Enables all tracing features. |
| `trace_pos` | Traces parser positions. |
| `trace_one_node` | Traces only the nearest grammar node for each attempt. |
| `trace_all_nodes` | Traces the complete grammar node chain for each attempt. |
| `lower_bnf_name` | Converts generated BNF names to lowercase. |
| `upper_bnf_name` | Converts generated BNF names to uppercase. |

## How it works

| Rust                               | Meaning                                  |
| ---------------------------------- | ---------------------------------------- |
| `struct`                           | sequence (`A B C`)                       |
| `enum`                             | alternation (`A \| B`)                   |
| `Vec<T>`                           | repetition (`{ T }`)                     |
| `Option<T>`                        | optional (`[ T ]`)                       |
| `StringEq!("…")`                   | literal string (`"…"`)                   |
| `StringEqCI!("…")`                 | case-insensitive literal string (`"…"i`) |
| `char_class!(Name, "…", \|c\| …)`  | define a character class (CC)            |
| `CharOf<CC>`                       | a character of the given character class |
| `StringOf<CC>` (`StringOf1<CC>`)   | (non-empty) sequence of characters       |
| `VecSep<T, S>`                     | separated list (`T { S T }`)             |
| `Either<A, B>`                     | inline alternation (`( A \| B )`)        |
| `(A, B)`                           | inline sequence (`A B`)                  |
| `Box<T>`                           | indirection (recursive rules)            |
| `Raw<T>`                           | parse via `T`, store matched `String`    |
| `NotFollowedBy<T>`                 | negative lookahead (`!T`)                |
| `Hidden<T>` / `#[grammar(hidden)]` | parse & print, but hide from EBNF        |
| `Wrap<L, T, R>`                    | `L T R`, dereferencing to `T`            |
| `Prefix<P, T>`                     | `P T`, dereferencing to `T`              |
| `Suffix<T, S>`                     | `T S`, dereferencing to `T`              |

## See also

**Deriving many routines from one spec.** Writing a grammar once and deriving several
routines from it is an old idea: [BNFC](https://bnfc.digitalgrammars.com/), for example,
turns a single labelled-BNF file into a parser, a pretty-printer, and a LaTeX document
of the grammar. tygr follows this spirit — one Rust type yields a parser, a printer, and
an EBNF presentation.

**Deriving parsers from grammars.** There are far too many parser libraries to list; a
few available to the Rust ecosystem, grouped by how the grammar is written:

| Project                                                                                                                                                  | Grammar is written as…         |
| -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| **tygr**                                                                                                                                                 | types                          |
| [gramma](https://github.com/youngspe/gramma)                                                                                                             | annotated types in a macro DSL |
| [nom](https://crates.io/crates/nom), [chumsky](https://crates.io/crates/chumsky), [combine](https://crates.io/crates/combine)                            | expressions                    |
| [pest](https://crates.io/crates/pest), [tree-sitter](https://tree-sitter.github.io/tree-sitter/) ([Rust bindings](https://crates.io/crates/tree-sitter)) | DSL                            |

Writing the grammar as types is what tygr trades on. A DSL lives outside the language,
so it misses the tooling — go-to-definition, refactoring, type checking — and composes
poorly with the rest of Rust (traits, generics, `#[derive]`, functions). Expression-based
combinators keep all of that, but you write the parsing expressions _and_ the types you
want the result in — two descriptions of the same structure to keep in sync. With tygr
the type _is_ the grammar, so there is nothing to keep in sync.

**PEG and parser combinators.** tygr reads a grammar as a recursive-descent parser with
backtracking, the reading refined by Parsing Expression Grammars
([PEG](https://en.wikipedia.org/wiki/Parsing_expression_grammar)) and parser combinators
([Parsec](https://hackage.haskell.org/package/parsec)).

## Contributing

Please [open an issue](../../issues/new/choose) before starting a pull request.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the development workflow.

## License

This software is provided **AS-IS**, with no implied warranties, support, or
conditions of any kind.

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
