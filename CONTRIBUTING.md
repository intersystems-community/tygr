# Contributing to tygr

Thanks for your interest in improving tygr!

## Open an issue first

Please open an issue before starting work on a pull request (PR). PRs without a linked issue will
be asked to open one. Submit PRs from a fork.

## Development

Standard Cargo workflow:

```sh
cargo build                                   # build
cargo test --all-targets                      # run tests
cargo test --doc                              # run doc tests
cargo fmt --all                               # format
cargo clippy --all-targets -- -D warnings     # lint
cargo doc --no-deps                           # build docs
```

CI runs `fmt --check`, `clippy`, `test`, doc tests, and `doc` across the
`default`, `--no-default-features`, and `--all-features` feature sets.

## License

By contributing, you agree that your contributions will be dual licensed under
the [Apache-2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT) licenses, without any
additional terms or conditions.
