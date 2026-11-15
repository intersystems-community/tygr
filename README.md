# 🐯 tygr: TYpe into Grammar Routines

Define your grammar once as Rust types and get a **parser**, **printer**, and **presentation in EBNF** for free.

## Get Started

Start with the crate documentation on [docs.rs](https://docs.rs/tygr): the API
and grammar concepts, plus a complete arithmetic grammar with parsing,
printing, and EBNF generation.

For a runnable example, see [`examples/json.rs`](examples/json.rs): the full
JSON grammar of json.org, with a small CLI around it:

```sh
cargo run --example json -- bnf              # print the grammar as EBNF
echo '{"a": [1, true]}' | cargo run --example json -- parse   # parse, print the AST
echo '{"a": [1, true]}' | cargo run --example json -- print   # parse, print it back
echo '{"a": 1}'         | cargo run --example json -- test    # check well-formed (exit 0/1)
```

## See Also

[Related work](RELATED_WORK.md): how tygr compares with other tools that derive
routines from a grammar, and with other Rust parser libraries.

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
