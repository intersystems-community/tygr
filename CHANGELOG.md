# Changelog

All notable changes to `tygr` and `tygr-derive` are documented here.

## [Unreleased]

### Breaking Changes

- A failed parse runs twice: the first pass tracks only the furthest
  position it reaches, and the second, in the new `Record<M>` mode, records
  what was expected there. The `trace`, `trace_pos`, `trace_one_node` and
  `trace_all_nodes` features are gone: every build has `Error::pos` and
  `Error::traces`, and each `Trace::context` is the whole chain of rules,
  outermost first. A successful parse does no tracing work: on the JSON
  benchmark (`canada.json`) it takes 16 ms instead of 25 with the default
  tracing, and a failed one 26 instead of 27.
- `State<M>` carries the mode, so a hand-written grammar takes
  `state: State<M>` and its calls to `node`, `reject` and `fail_at` infer it;
  `Parse::fail_at` is generic over the mode like `parse_at`. `Mode` has
  `RECORD`, `Built`, `Checked` and `built`, and `Parse` has `build_at` and
  `check_at` (`State::built` and `checked` underneath), to parse a part
  building or only checking it whatever the mode.
- `OLC::offset_to_line_character` and `line_character_to_offset` return a
  `Result`: a position past the input or a line, or inside a char or a
  surrogate pair, is an `OutOfRange { bound, excess }`, the nearest valid
  value and how far past it the position was, rather than being clamped
  silently. `unwrap_or_else(|e| e.bound)` clamps as 0.7 did.
- `Expectation::Rejected`'s `requirement` is a `Cow<'static, str>`, and
  `State::reject` takes any `Into<Cow<'static, str>>`: a hand-written
  grammar can reject with a message built from what it parsed
  (`format!(..)`), which the error carries. Recording a `&'static str`
  costs what it did.

### Non-breaking Changes

- A conversion on the type itself: `#[tygr(try_from = S, print_to = f)]`
  (or `try_from_infallible`, `from_str`, `from_str_infallible`) on a
  struct or enum parses `S` and converts it into the whole value, as on a
  field, `f` taking `&Self`. Its fields or variants are the conversion's
  to fill rather than parts of the grammar, so a type built from a grammar
  (an enum of keywords from a word rule, a struct from a sequence) needs
  no wrapper struct. `name`, `hidden`, `inline`, `validated` and
  `unbounded` combine with it. This is 0.6's `GrammarFromStr`,
  `GrammarFromOther` and `GrammarTryFromOther` as `from_str`,
  `try_from_infallible` and `try_from`, with `print_to` for
  `GrammarFrom::print_to`. The derive's error for an unknown option on a
  type now names every option, `unbounded` included.

## [0.7.0] - 2026-11-12

Literals, FIRST sets and enum dispatch now match on bytes, under grammars
that still see `&str`. On the JSON benchmark (`canada.json`), parsing is
over 12× faster with the default tracing and over 3× faster without it.

### Breaking Changes

- The derive attribute is `#[tygr(...)]`, not `#[grammar(...)]`.
- `Grammar` is split into `Parse` (parsing and EBNF) and `Print`.
  `Grammar` is now `Parse + Print`, implemented for every type with both.
  `#[derive(Grammar)]` derives both; `#[derive(Parse)]` and
  `#[derive(Print)]` derive one, e.g. a grammar that is only parsed
  needs no printing.
- Removed `GrammarFrom` and its derives (`GrammarFromStr`,
  `GrammarFromOther`, `GrammarTryFromOther`). Convert a field instead,
  with `print_to` where `GrammarFrom::print_to` was:
  `struct Port(#[tygr(from_str = Digits, print_to = print_display)] u16);`.
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
- `PhantomData<T>` no longer implements `Print` (it kept nothing to print
  back): a type with such a field derives `Parse` alone, or prints the
  field with `#[tygr(print_to = f)]`.
- Renamed `Range<T>` to `Ranged<T>`, freeing the name for
  `std::ops::Range`. It gains `range() -> Range<usize>`.
- `StringEqCI!` matches each char's Unicode case mappings, and theirs,
  rather than only ASCII letters: `"é"` matches `"É"`, and `"ß"` matches
  `"SS"` and `"ss"`.
- A literal that fails partway is still reported where it diverged, but
  as the whole literal with the range it matched, as a rejected
  validation is: `"->"` against `"-x"` reads `"->" at 0..1` at byte 1,
  not `"->"'s ">"`.
- `Expectation`:
  - `StringEq` and `StringEqCI` merged into `Token { text, range }`.
  - `Valid` and `GrammarFrom` merged into
    `Rejected { node, range, requirement }`. It records the rejected byte
    range instead of its text; slice your input to recover it. A failed
    conversion has no `node`, and the requirement `"be convertible to T"`
    for a field of type `T` rather than the conversion error's message.
  - `Display` changed to match: `"->"` (or `"->" at 0..1` once partly
    matched), `NonZero at 0..3 must be non-zero`, `Text at 0..3 must be
    convertible to u8`. A character class shows as in EBNF, `'digit'`,
    not `Char of digit`.
  - It's `#[non_exhaustive]`, and gains `End` and
    `NotFollowedBy { excluded, range }`: the excluded grammar's EBNF, a
    `bnf::Lazy`, and what it matched.
- `Error`'s fields are private: read them with `pos()` and `traces()`. Its
  `Display` no longer ends with a newline.
- `FollowedBy<G>` is its own type rather than
  `NotFollowedBy<NotFollowedBy<G>>`, so when it fails it reports what `G`
  expected.
- `bnf::Expr::StringEq` and `StringEqCI` merged into
  `Expr::Token { text, case_insensitive }`. A case-insensitive literal
  without cased chars now prints without the `i` (`"123"`). `Expr::CharOf`
  is now `Expr::CharClass`, as in `Expectation`.
- `bnf::Expr` gains `Hidden`. A hidden alternative or lookahead is
  dropped from EBNF, rather than making the rest optional (`[ A ]`).
- Reworked `tygr::first` around bytes, for hand-written `Parse::First`:
  - Name a constant set with `Required<S>` (not nullable) or `Optional<S>`
    (nullable), where `S` is built from `byte_set::{Empty, All, One,
    Between, Union}`.
  - Combine sets with `First::Seq<G>` (`G` a grammar), `First::Alt<X>`
    and `First::Opt`.
  - Query a set with `first::contains::<F>(next: Option<u8>)`.
  - Removed `Never`, `AnyChar`, `WithChar`, `WithCharCI`, `Nullable`,
    `CONTAINS_BYTE` and `CONTAINS_NIL`.
- `State::expect` and `probe` are crate-private. `State` is documented for
  hand-written grammars, which record a failed check with the new
  `State::reject` and otherwise delegate to their parts' `fail_at`.
- The types that `StringEq!`/`StringEqCI!` expand to (`StringEq`,
  `StringEqCI`, `CharThen`, `CharCIThen`) are gone. The macros now expand
  to internal types; use the macros rather than naming their output.
- `OLC` breaks lines at `\r\n` and `\r` too, and `line_character_to_offset`
  clamps out-of-range positions, as LSP does, and maps one inside a
  surrogate pair to the pair's start.
- Removed `VecSep`. Write `T { S T }` as `(T, Vec<(S, T)>)`.

### Non-breaking Changes

- Removed `examples/json_optimized.rs`. `examples/json.rs` is the one JSON
  example and gains a `print` command that prints its input back;
  `cargo bench --bench json` benchmarks the same grammar.
- `char` is a grammar for any one char.
- Field conversions: `#[tygr(<kind> = S)]` on a field parses grammar `S`
  and converts it with `try_from` (`S: TryInto<T>`), `try_from_infallible`
  (`Error = Infallible`, as for every `From`), `from_str` (`T: FromStr`
  on the matched text) or `from_str_infallible`. A failed conversion
  backtracks and is recorded as `Rejected`; an infallible one is skipped
  when checking. Printing a converted field needs `print_to = f`, such as
  the new `print_display`. A unit variant takes `try_from` or
  `try_from_infallible`, converting its source (e.g. `StringEq!("null")`)
  into `()`; its `print_to` takes `&()`.
- `Prefix` and `Suffix` fields are public, like `Wrap`'s, and so are
  `Hidden`'s and `Ranged`'s. `Vec1` gains `IntoInner` and `TryFrom<Vec<T>>`.
  `Raw<T>` defaults to the text `T::default()` prints (empty for
  `StringOf`), and `Hidden`, `Wrap`, `Prefix` and `Suffix` derive `Default`.
  `Wrap`, `Prefix`, `Suffix`, `Vec1`, `Ranged`, literals and
  `char_class!` types are `Hash`, so a grammar holding them can derive it.
  Every combinator can be built by hand, e.g. to print.
- `OLC` is `Debug` and `Clone`, so a type holding one can derive them.
- `Error`, `Trace` and `Expectation` are `Clone`; `Error` implements
  `std::error::Error`, and `Expectation` is `Display` whatever the features.
- EBNF: a character class's name escapes only `'`, not `"`; a hidden
  conversion source drops its footnote; an empty rule prints as `X = .`;
  an empty enum, which never matches, is footnoted `never matches`;
  a footnoted factor in a sequence isn't parenthesized (`Digits ^1 ","
  Digits ^2`); an empty operand of a footnote or lookahead prints as
  `( )`.
- The name argument of `char_class!` is optional and defaults to the type
  name.
- Fixed derived code failing to compile unless tygr's traits were in
  scope (e.g. with `use tygr::Grammar;` instead of `use tygr::*;`), or
  when the prelude's names (`Option`, `Vec`, `stringify!`) were shadowed
  or absent.
- Failed literals, validations and conversions no longer allocate when
  recording what was expected, nor lookaheads when probing.
- `Error::traces()` has no duplicates. A backtracking grammar can expect
  the same few things at one position exponentially often:
  `S = "a" S "b" | "a" S "c" | ()` on 22 `a`s reported 12 million traces,
  now three.
- The crate is `#![forbid(unsafe_code)]`. A hand-written grammar that ends
  mid-char now finds no char there, rather than panicking as 0.6.2's
  `CharOf` did; a `Raw` or `from_str` conversion keeping such text still
  panics.
- Errors report input left over after a match, as `Expectation::End`,
  and a failed negative lookahead, as `NotFollowedBy`; both recorded
  nothing.
- An enum of four or more variants reports every variant that could have
  matched, as a smaller one does; the variants its dispatch table skipped
  went unreported.
- Derived impls bound every type parameter with `Parse` (and `Print`),
  as std's derives do; `#[tygr(unbounded(T))]` leaves `T` with only its
  own bounds, for a conversion target or a trait carrier whose associated
  type is the grammar. Deriving works for an empty
  enum, and a recursive `#[tygr(inline)]` type gets an EBNF rule of its own
  rather than recursing forever.
- Declared the minimum supported Rust version, `rust-version = "1.88"`,
  which CI builds on. Raising it comes only with a 0.x → 0.y release.
- A field whose type is no grammar fails to derive with `` `u16` doesn't
  parse as a grammar `` and a note pointing to `#[tygr(from_str = S)]` or
  `try_from`.
- `Error`'s `Debug` output opens `parse error at byte N, expected:`, as
  its `Display` does, and writes each context frame as `Rule@pos` rather
  than `Rule@:pos`.

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
