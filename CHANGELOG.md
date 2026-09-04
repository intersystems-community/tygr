# Changelog

All notable changes to `tygr` and `tygr-derive` are documented here.

## [0.6.2] - 2026-09-04

### Non-breaking Changes

- Reworded `StringEq!`/`StringEqCI!` partial-match `Display` from
  `"partial" of "full"` to a possessive, `"full"'s "partial"` (e.g.
  `"->"'s ">"` instead of `">" of "->"`), which reads in the right
  direction.
- Quoted and escaped `GrammarFrom`'s matched text in `Display`,
  matching how `Valid`'s `text` field (the same kind of raw matched
  text) is already treated.

## [0.6.1] - 2026-09-04

### Non-breaking Changes

- Fixed `StringEq!`/`StringEqCI!` partial matches (e.g. matching `"-"`
  of `"->"` then failing on `>`) reporting the failure at the start of
  the literal instead of where the mismatch actually occurred — a
  regression introduced alongside 0.6.0's `Expectation::StringEq`/
  `StringEqCI` fix.

## [0.6.0] - 2026-09-04

### Breaking Changes

- Changed `Expectation::StringEq`/`StringEqCI` from a single `String` field
  to two (`partial`, `full`) — a `StringEq!`/`StringEqCI!` literal that
  fails partway through (e.g. matching `"-"` of `"->"` then failing on
  `>`) previously recorded only the unmatched tail (`">"`), losing the
  whole literal it was actually trying to match. `Display` now shows
  `"partial" of "full"` when they differ, and just the literal (as
  before) when they don't.

## [0.5.0] - 2026-09-03

### Breaking Changes

- Changed `OLC::offset_to_line_character`/`line_character_to_offset`'s
  `character` from a byte offset to a UTF-16 code-unit offset, matching
  LSP's `Position.character` — a silent behavior change (not a compile
  error) for any line containing non-ASCII text.

### Non-breaking Changes

- Added `Prefix::new(prefix, prefixed)` and `Suffix::new(suffixed, suffix)`,
  matching `Wrap::new`'s explicit-all-fields shape.
- Added `Debug`, `Clone`, `PartialEq`, `Eq` derives to `Vec1<T>`, matching
  `Range`/`VecSep`/`Wrap` — it previously had none of the four.

## [0.4.0] - 2026-08-24

### Breaking Changes

- Changed `VecSep<T, S>`'s representation from `head`/`rest` fields with
  `items()`/`seps()` accessor methods to public `items: Vec<T>` and
  `seps: Vec<S>` fields, giving direct (including mutable) access.
- Moved the FIRST-set types out of the crate root into a new `first`
  module, and renamed them for readability: `First::UEmpty` →
  `First::Nullable`, `First::UChar`/`UCharCI` → `First::WithChar`/
  `WithCharCI`, `EmptyByteSet` → `first::Never`, `AnyCharFirst` →
  `first::AnyChar`. `ByteSet` is no longer public — it was internal
  plumbing manual `impl Grammar`s never needed. Documented, on
  `Grammar::First`, the three ways a manual `impl Grammar` picks its
  `First`.
- Simplified `Validate`: dropped the `Validation` trait and the
  `type Result: Validation` associated type — `validate()` now returns
  a plain `bool`, and the rejection message is a mandatory
  `const REQUIREMENT: &'static str` on the trait itself, shown in both
  error traces and generated BNF (no default — it's user-facing
  documentation, not just an error detail, so every `impl Validate`
  must supply something concrete). Renamed `Expectation::Valid`'s
  `be_valid` field to `requirement`, and reworded its `Display` output
  from "The proceeding {node} \"{text}\" to {be_valid}" to
  "{node} \"{text}\" must {requirement}".

### Non-breaking Changes

- Added `bnf::Expr::SideCondition`, and wired it into generated BNF: a
  `#[grammar(validated)]` type's own rule definition now shows a
  numbered marker (`^1`, `^2`, ...) with `Validate::REQUIREMENT`, and a
  fallible `GrammarFromStr`/`GrammarTryFromOther` conversion's shows one
  with "be convertible" (`GrammarFromOther`'s `From` can't fail, so it's
  unaffected) — listed as a footnote after the rule. Footnotes are
  deduplicated by string equality within a rule, so the same
  side-condition repeated (e.g. via `#[grammar(inline)]`) gets one
  shared marker. The generic "be convertible" text is BNF-only — the
  trace for a failed conversion still shows just the real conversion
  error's message, since a repeated generic requirement there would add
  nothing.
- `#[grammar(validated)]` is no longer rejected on `GrammarFromStr`/
  `GrammarFromOther`/`GrammarTryFromOther` — `Validate::validate` now
  runs after a successful conversion on any of them, independent of
  whether the conversion itself can fail. `GrammarFromOther`'s `scan_at`
  now runs the full check via `parse_at` when `validated`, rather than
  delegating straight to `Source::scan_at`.
- Added `FollowedBy<G>`, the positive-lookahead counterpart to
  `NotFollowedBy<G>`.
- Fixed BNF output showing empty `[ ]`/`{ }` brackets for an
  `Option`/`Vec` wrapping an entirely-hidden element, instead of omitting
  it like the hidden element itself would be.

## [0.3.0] - 2026-08-21

### Breaking Changes

- Added a `GrammarRule` impl for `GrammarFromStr`/`GrammarFromOther`/
  `GrammarTryFromOther`, giving them a real `NAME` and
  `#[grammar(name = "...", hidden, inline)]` support; as a result they now
  reference their own BNF rule by name by default instead of always
  splicing `Source`'s definition inline (`#[grammar(inline)]` restores the
  old behavior). `#[grammar(validated)]` is rejected there.
- Changed `Expectation::Valid`'s fields — dropped `pos` (and the
  `Expectation::pos()` accessor); added `node` (the rejected type's
  `NAME`) and `text` (the raw rejected span).
- Changed `Expectation::GrammarFrom` from a tuple variant to a struct
  variant with `from` (raw text that failed to convert), `into` (target
  type's `NAME`), and `fail` (the conversion error's message).
- Changed `Display for Error`/`Expectation` wording to consistently
  report every trace's node name and full expectation text, instead of a
  heuristic "nearest node OR raw expectation" choice.
- Changed `bnf_rule()`'s rule terminator from `;` to `.`, matching the
  cited Wirth syntax notation exactly.

### Non-breaking Changes

- Added `Grammar::parse_prefix`, like `parse()` but without requiring the
  whole input to match — returns the byte position just past the match.
- Added `Grammar::scan_prefix`, the `scan()` counterpart to
  `parse_prefix`.
- Cited Wirth syntax notation (Wirth, 1977) in the crate-level Design
  table, replacing the "ISO 14977" citation, which was inaccurate (that
  standard requires `,` for concatenation; the `Display` impl never
  emits one).
- Fixed inconsistent EBNF snippets in doc comments that used commas, to
  match the whitespace concatenation the library actually outputs.
- Removed a stale, duplicate construct-mapping table from `grammar.rs`'s
  module doc that referenced a nonexistent `write_bnf(w)` method.

## [0.2.1] - 2026-08-19

### Non-breaking Changes

- Fixed enum dispatch miss-branch tracing being silently dead for any
  `#[derive(Grammar)]` outside the `tygr` crate itself — an embedded
  `#[cfg(feature = "trace_pos")]` was checking the deriving crate's own
  features instead of `tygr`'s
  ([#5](https://github.com/intersystems-community/tygr/issues/5)).

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
