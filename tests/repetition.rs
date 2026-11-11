//! `Vec`, `Vec1` and `VecSep` end at an item that matches empty, rather than looping.

use tygr::*;

char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct MaybeAs(Vec<Option<StringEq!("a")>>);

#[derive(Grammar, Debug)]
struct OneMaybe(Vec1<Option<StringEq!("a")>>);

// The separator is nullable too: one that must consume advances the loop by itself.
#[derive(Grammar, Debug)]
struct SepOpt(VecSep<Option<Digit>, Option<StringEq!(",")>>);

#[test]
fn an_item_matching_empty_ends_the_repetition() {
    assert_eq!(MaybeAs::parse("aa").unwrap().0.len(), 2);
    assert_eq!(MaybeAs::parse("").unwrap().0.len(), 0);
    assert!(OneMaybe::parse("").is_err());
    assert_eq!(OneMaybe::parse("a").unwrap().0.len(), 1);
    let list = SepOpt::parse("1,2").unwrap().0;
    assert_eq!((list.items.len(), list.seps.len()), (2, 1));
}
