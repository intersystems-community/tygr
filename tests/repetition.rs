//! `Vec` and `Vec1` end at an item that matches empty, rather than looping.

use tygr::*;

char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug)]
struct MaybeAs(Vec<Option<StringEq!("a")>>);

#[derive(Grammar, Debug)]
struct OneMaybe(Vec1<Option<StringEq!("a")>>);

// Separated, with the separator nullable too: a pair matching empty ends it.
#[derive(Grammar, Debug)]
struct SepOpt(Option<Digit>, Vec<(Option<StringEq!(",")>, Option<Digit>)>);

#[test]
fn an_item_matching_empty_ends_the_repetition() {
    assert_eq!(MaybeAs::parse("aa").unwrap().0.len(), 2);
    assert_eq!(MaybeAs::parse("").unwrap().0.len(), 0);
    assert!(OneMaybe::parse("").is_err());
    assert_eq!(OneMaybe::parse("a").unwrap().0.len(), 1);
    let list = SepOpt::parse("1,2").unwrap();
    assert_eq!((list.0.is_some(), list.1.len()), (true, 1));
}
