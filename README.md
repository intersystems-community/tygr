# 🐯 tygr: TYpes Into Grammar Routines

Define your grammar once as Rust types and get **parser**, **printer**, and **presentation in EBNF** for free.

## Get Started

Start with the crate documentation on [docs.rs](https://docs.rs/tygr), which
contains a complete arithmetic grammar with parsing, printing, and EBNF generation.

See the crate documentation for the complete API, grammar concepts, and feature
flags.

For runnable examples, see:

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

## See Also

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
