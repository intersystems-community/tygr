# Contributing to tygr

Thanks for your interest in improving tygr!

## Open an issue first

Please open an issue before starting work on a pull request (PR). PRs without a linked issue will
be asked to open one. Submit PRs from a fork.

## Development

Standard Cargo workflow:

```sh
cargo build                                   # build
cargo test                                    # run tests, doc tests included
cargo fmt --all                               # format
cargo clippy --workspace --all-targets -- -D warnings  # lint both crates
cargo doc --no-deps                           # build docs
```

CI runs `fmt --check`, `clippy`, `test`, doc tests, and `doc` across the
`default`, `--no-default-features`, and `--all-features` feature sets and
each `trace_*` feature alone, builds on the `rust-version` in Cargo.toml,
and checks that both crates' versions and `Cargo.lock` line up.

## Releasing

`tygr` and `tygr-derive` are versioned in lockstep. To publish a new release:

```sh
# 1. Bump `version` in Cargo.toml and tygr-derive/Cargo.toml, and the `tygr-derive`
#    dependency's `version` in Cargo.toml, to X.Y.Z; retitle CHANGELOG's `[Unreleased]`.
# 2. Refresh Cargo.lock, which CI checks:
cargo check
# 3. Open that as a PR; once it's squash-merged, tag the release commit on master:
git switch master && git pull
git tag vX.Y.Z
git push origin vX.Y.Z
```

Pushing the tag triggers [`.github/workflows/publish.yml`](.github/workflows/publish.yml),
which checks the tag against both `Cargo.toml` versions, runs the test suite,
then publishes `tygr-derive` before `tygr` (in that order, since `tygr`
depends on it).

## License

By contributing, you agree that your contributions will be dual licensed under
the [Apache-2.0](LICENSE-APACHE) and [MIT](LICENSE-MIT) licenses, without any
additional terms or conditions.
