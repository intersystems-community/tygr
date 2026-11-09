//! Where a failed parse stops, whenever positions are traced.
#![cfg(feature = "trace_pos")]

use tygr::*;

#[derive(Parse, Debug)]
struct Lit(StringEq!("foo"));

#[derive(Parse, Debug)]
struct Pair(Lit, StringEq!(","), Lit);

#[derive(Parse, Debug)]
struct Div(StringEq!("/"), NotFollowedBy<StringEq!("/")>);

#[derive(Parse, Debug)]
struct Key(StringEq!("k"), FollowedBy<StringEq!(":")>);

#[test]
fn errors_stop_where_parsing_did() {
    assert_eq!(Lit::parse("bar").unwrap_err().pos(), 0);
    assert_eq!(Lit::parse("foox").unwrap_err().pos(), 3);
    assert_eq!(Lit::check("foox").unwrap_err().pos(), 3);
    assert_eq!(Pair::parse("foo,fob").unwrap_err().pos(), 4);
    assert_eq!(Div::parse("//").unwrap_err().pos(), 1);
    assert_eq!(Key::parse("k").unwrap_err().pos(), 1);
}
