# Changelog

All notable changes to `tygr` and `tygr-derive` are documented here.

## [0.2.0] - 2026-08-19

### Breaking Changes

- Added a `Validate` trait, replacing the ad hoc `filter()` method.
- Added `Grammar::fail_at(pos, state) -> bool`, required on manual
  `impl Grammar` blocks, to report what a grammar would have expected at
  `pos` without actually parsing.
- Changed the default feature from `trace` to `trace_one_node`.
- Changed `Attempt`'s `context` field from `Vec<(&'static str, usize)>` to
  `Vec<Frame>`.
- Fixed enum dispatch silently dropping earlier variants' failed-match
  traces, reporting only the last variant's expectation.
- Fixed `PhantomData<T>::print_to` panicking; it now prints nothing.
- Removed `bnf::Expr::Empty`; represented empty as an empty `Sequence`
  instead (`Expr::empty()`).
- Removed unused `AnyThingFirst`.
- Renamed `#[grammar(filtered)]` to `#[grammar(validated)]`, and
  `#[grammar(transparent)]` to `#[grammar(inline)]`.
- Renamed `FilterResult` trait to `Validation`.
- Renamed `Attempt` to `Trace`.
- Renamed `Expectation::Filtered` to `Valid`, and `Expectation::Conversion`
  to `GrammarFrom`.
- Renamed `bnf::Expr::Literal`/`LiteralCI` to `StringEq`/`StringEqCI`.
- Replaced the `trace_attempts`/`trace_any`/`history`/`context` feature
  flags with `trace_pos`/`trace_one_node`/`trace_all_nodes`.
- Restricted `CharFirst`, `CharFirstCI`, `EmptyFirst` to `pub(crate)` (they
  were public, unintentionally).
- Stopped merging adjacent literal tokens into a single quoted string when
  printing BNF.

### Non-breaking Changes

- Added rustdoc coverage for `Expectation`, `Trace`, `Frame`, `Validate`,
  `IntoInner`, `OLC`, and the derive macros, which previously had none.
- Exposed `lower_bnf_name`/`upper_bnf_name` features from the root `tygr`
  crate too (previously only from `tygr-derive`).
- Fixed `#[grammar(...)]` silently ignoring malformed attribute arguments
  (e.g. `#[grammar(bogus)]` compiled with no error); parse errors now
  propagate.
- Removed `examples/arith.rs`, superseded by the crate-level Quick Example
  doctest.

## [0.1.0] - 2026-08-18

Initial release.
