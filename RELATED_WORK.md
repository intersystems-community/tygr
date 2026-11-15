# Related Work

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

**PEG and parser combinators.** tygr reads a grammar as a backtracking recursive-descent
parser, in the manner of Parsing Expression Grammars
([PEG](https://en.wikipedia.org/wiki/Parsing_expression_grammar)) and parser combinators
([Parsec](https://hackage.haskell.org/package/parsec)).
