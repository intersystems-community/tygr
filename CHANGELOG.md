# Changelog

All notable changes to `tygr` and `tygr-derive` are documented here.

## [Unreleased]

The parser now works on bytes, with string and character grammars built
on top. On the JSON benchmark (`canada.json`), parsing is about 12× faster
with the default tracing (6.6 → 80 MB/s) and about 3.5× faster without it
(33 → 114 MB/s).

### Breaking Changes

- The derive attribute is `#[tygr(...)]`, not `#[grammar(...)]`.
- `Grammar` is split into `Parse` (parsing and BNF) and `Print`.
  `Grammar` is now `Parse + Print`, implemented for every type with both.
  `#[derive(Grammar)]` derives both; `#[derive(Parse)]` and
  `#[derive(Print)]` derive one, e.g. a grammar that is only parsed
  needs no printing.
- Removed `GrammarFrom` and its derives (`GrammarFromStr`,
  `GrammarFromOther`, `GrammarTryFromOther`). Convert a field instead:
  `struct Port(#[tygr(from_str = Digits)] u16);`.
- `scan` and `scan_prefix` are renamed `check` and `check_prefix`.
- Removed `GrammarRule` and `bnf_rules!`. `Parse::bnf_rules()` collects a
  grammar's rules, each type's once, depth first, as `bnf::Rule`s to sort,
  `rename` and print. Hand-written grammars implement `visit_rules`.
- `bnf::Expr::format` is replaced by `Display`.
- Removed the `lower_bnf_name` and `upper_bnf_name` features, as features
  should be additive. Rename rules with `Rule::rename` instead.
- `Grammar::scan_at` is merged into `Parse::parse_at::<M: Mode>`, which
  returns `M::Output<Self>`: the value in mode `Build`, `()` in mode
  `Check`. Hand-written grammars build outputs with `Mode::build`/`map`.
- `parse_at` takes `input: &[u8]`, the UTF-8 bytes of the `&str` passed
  to `parse` and friends (whose signatures are unchanged). Every position
  is a char boundary; hand-written grammars must keep it that way.
- `Parse` is an `unsafe trait`, as tygr's UTF-8 conversions rely on that
  rule: a hand-written grammar is an `unsafe impl Parse`. Derived and
  `char_class!` impls need no change, even under `#![forbid(unsafe_code)]`.
- `Print::print_to` writes to `&mut Vec<u8>`. `print()` still returns a
  `String`.
- Removed `CharOf<M>`: a `char_class!` type is now itself the grammar for
  one matching char (`struct IsDigit(pub char)`). Replace `CharOf<IsDigit>`
  with `IsDigit`. `CharClass` now requires `Grammar` and gains `new` and
  `value`; define classes with `char_class!` rather than by hand.
- `char_class!` requires a `const` predicate, so the class's first bytes
  are known at compile time and enums can dispatch on them. Most
  predicates already qualify (`is_ascii_*`, `is_whitespace`, `matches!`);
  write `matches!(c, 'a'..='f')` rather than `('a'..='f').contains(&c)`.
  For a predicate that can't be `const`, such as `char::is_alphabetic`,
  use a `#[tygr(validated)]` type wrapping `char`.
- `StringOf<C>` and `StringOf1<C>` are type aliases for `Raw<Vec<C>>` and
  `Raw<Vec1<C>>`, so they `Debug`-print as `Raw(..)`.
- `PhantomData<T>` prints `T::default()` rather than nothing, so a dropped
  token prints back; printing it needs `T: Default`. `Raw<T>` defaults to
  the text `T::default()` prints (empty for `StringOf`), and `Hidden`,
  `Wrap`, `Prefix` and `Suffix` derive `Default`.
- Renamed `Range<T>` to `Ranged<T>`, freeing the name for
  `std::ops::Range`. It gains `range() -> Range<usize>`.
- `StringEqCI!` matches each char's Unicode case mappings rather than only
  ASCII letters: `"é"` matches `"É"`, and `"ß"` matches `"SS"`.
- A literal that fails to match is reported as a whole, at the position
  where it starts. This reverts 0.6.1, which reported partial matches
  where they diverged: `"->"` against `"-x"` now fails at byte 0 instead
  of byte 1.
- `Expectation`:
  - `StringEq` and `StringEqCI` merged into `Token(&'static [u8])`.
  - `Valid` and `GrammarFrom` merged into
    `Rejected { node, range, requirement }`. It records the rejected byte
    range instead of its text; slice your input to recover it. A failed
    conversion has the requirement `"be convertible"` and no longer
    carries the conversion error's message.
  - `Display` changed to match: `"->"`, `NonZero at 0..3 must be
    non-zero`, `SmallNumber at 0..3 must be convertible`.
  - It's `#[non_exhaustive]`, and gains `NotFollowedBy(range)` and `End`.
- `Error`'s fields are private: read them with `pos()` and `traces()`. Its
  `Display` no longer ends with a newline.
- `FollowedBy<G>` is its own type rather than
  `NotFollowedBy<NotFollowedBy<G>>`, so when it fails it reports what `G`
  expected.
- `bnf::Expr::StringEq` and `StringEqCI` merged into
  `Expr::Token { text, case_insensitive }`. A case-insensitive literal
  without cased chars now prints without the `i` (`"123"`).
- `bnf::Expr` gains `Hidden`. A hidden alternative or lookahead is
  dropped from BNF, rather than making the rest optional (`[ A ]`).
- Reworked `tygr::first` around bytes, for hand-written `Parse::First`:
  - Name a constant set with `Required<S>` (not nullable) or `Optional<S>`
    (nullable), where `S` is built from `byte_set::{Emp, All, One,
    Between, Union}`.
  - Combine sets with `First::Seq<G>` (`G` a grammar), `First::Alt<X>`
    and `First::Opt`.
  - Query a set with `first::contains::<F>(next: Option<u8>)`.
  - Removed `Never`, `AnyChar`, `WithChar`, `WithCharCI`, `Nullable`,
    `CONTAINS_BYTE` and `CONTAINS_NIL`.
- The types that `StringEq!`/`StringEqCI!` expand to (`StringEq`,
  `StringEqCI`, `CharThen`, `CharCIThen`) are gone. The macros now expand
  to internal types; use the macros rather than naming their output.

### Non-breaking Changes

- Removed `examples/json_optimized.rs`. `examples/json.rs` is the one JSON
  example, and `cargo bench --bench json` benchmarks the same grammar. It
  gains a `print` command that prints its input back.
- `char` is a grammar for any one char.
- Field conversions: `#[tygr(<kind> = S)]` on a field parses grammar `S`
  and converts it with `try_from` (`S: TryInto<T>`), `try_from_infallible`
  (`Error = Infallible`, as for every `From`), `from_str` (`T: FromStr`
  on the matched text) or `from_str_infallible`. A failed conversion
  backtracks and is recorded as `Rejected`; an infallible one is skipped
  when checking. The field prints with its type's `Print`, or with
  `write_to = f`, such as the new `write_display`. On a unit variant, any
  kind matches `S` and keeps nothing.
- `Prefix` and `Suffix` fields are public, like `Wrap`'s, and so is
  `Hidden`'s. `Vec1` gains `IntoInner` and `TryFrom<Vec<T>>`. Every
  combinator can be built by hand, e.g. to print.
- `Error` implements `std::error::Error`, and `Expectation` is `Display`
  whatever the features.
- BNF: a character class's name escapes only `'`, not `"`; a hidden
  conversion source drops its footnote; an empty rule prints as `X = .`.
- `VecSep` reserves the usual amortized capacity on its first item, rather
  than exactly one, so short lists no longer reallocate.
- The name argument of `char_class!` is optional and defaults to the type
  name.
- Fixed derived code failing to compile unless tygr's traits were in
  scope (e.g. with `use tygr::Grammar;` instead of `use tygr::*;`).
- Failed literals, validations and conversions no longer allocate when
  recording what was expected, nor lookaheads when probing.
- Errors report input left over after a match, as `Expectation::End`,
  and a failed negative lookahead, as `NotFollowedBy`; both recorded
  nothing.
- An enum of four or more variants reports every variant that could have
  matched, as a smaller one does, rather than only those from the first
  that could start with the next byte.
- Derived impls bound only the type parameters their fields parse or
  print, so a converted field can be generic. Deriving works for an empty
  enum, and a recursive `#[tygr(inline)]` type gets a BNF rule of its own
  rather than recursing forever.
- `OLC` breaks lines at `\r\n` and `\r` too, and `line_character_to_offset`
  clamps out-of-range positions, as LSP does, and maps one inside a
  surrogate pair to the pair's start.
- Declared `rust-version = "1.88"`.

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
