# 🐯 tygr: TYpe into Grammar Routines

Define your grammar once as Rust types and get a **parser**, **printer**, and **presentation in EBNF** for free.

## Get Started

Start with the crate documentation on [docs.rs](https://docs.rs/tygr): the API
and grammar concepts, plus a complete arithmetic grammar with parsing,
printing, and EBNF generation.

For a runnable example, see [`examples/json.rs`](examples/json.rs): the full
JSON grammar of json.org, with a small CLI around it. The grammar prints
itself as EBNF:

```console
$ cargo run --example json -- bnf
Json = Ws Value Ws .
Ws = { " " | "\n" | "\r" | "\t" } .
Value = Object | Array | String | Number | "true" | "false" | "null" .
Object = "{" Ws [ Members ] "}" .
Members = Member Ws { "," Ws Member Ws } .
Member = String Ws ":" Ws Value .
String = "\"" Characters "\"" .
Characters = { Character } .
Character = '0x20-0x10FFFF except for " and \\' | "\\" Escape .
Escape = "\"" | "\\" | "/" | "b" | "f" | "n" | "r" | "t" | "u" Hex Hex Hex Hex .
Hex = Digit | 'A-F' | 'a-f' .
Digit = "0" | OneNine .
OneNine = '1-9' .
Array = "[" Ws [ Elements ] "]" .
Elements = Value Ws { "," Ws Value Ws } .
Number = Integer [ Fraction ] [ Exponent ] .
Integer = [ "-" ] ( OneNine Digits | Digit ) .
Digits = Digit { Digit } .
Fraction = "." Digits .
Exponent = ( "E" | "e" ) [ Sign ] Digits .
Sign = "+" | "-" .
```

It parses input into a syntax tree, a value of the grammar's types:

```console
$ echo '[1]' | cargo run --example json -- parse
Json(
    Wrap {
        before: Ws(
            [],
        ),
        wrapped: Array(
            ...
```

It prints a tree back, and checks an input, exiting 0 or 1:

```console
$ echo '{"a": [1, true]}' | cargo run --example json -- print
{"a": [1, true]}
$ echo '{"a": 1}' | cargo run --example json -- test
ok
$ echo '{"a" 1}' | cargo run --example json -- test
parse error at byte 5, expected:
	- " "
	- ":"
	- "\n"
	- "\r"
	- "\t"
```

## Related Work

[RELATED_WORK.md](RELATED_WORK.md) places tygr among the tools that derive several
routines from one grammar, and among Rust parser libraries by how their grammars
are written.

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
