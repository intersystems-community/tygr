//! A leaf grammar implemented by hand, through the public API.

use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use tygr::bnf::{Expr, Rules};
use tygr::*;

#[derive(Debug)]
struct Letter(char);

impl Parse for Letter {
    type First = <char as Parse>::First;

    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let mut state = state.node("Letter", pos);
        let (ch, end) = char::build_at(input, pos, state.reborrow())?;
        if !ch.is_alphabetic() {
            state.reject("Letter", pos..end, "be a letter");
            return None;
        }
        Some((M::build(|_| Letter(ch)), end))
    }

    fn fail_at<M: Mode>(pos: usize, mut state: State<M>) -> bool {
        char::fail_at(pos, state.node("Letter", pos))
    }

    fn to_bnf() -> Expr {
        Expr::RuleRef("Letter".into())
    }

    fn visit_rules(rules: &mut Rules) {
        if rules.insert::<Self>("Letter", || {
            Expr::side_condition(char::to_bnf(), "be a letter")
        }) {
            char::visit_rules(rules);
        }
    }
}

impl Print for Letter {
    fn print_to(&self, buf: &mut String) {
        buf.push(self.0);
    }
}

#[derive(Grammar, Debug)]
struct Word(Vec1<Letter>);

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

/// Four variants dispatch on a table, which reports skipped ones with `fail_at`.
#[derive(Parse, Debug)]
enum Item {
    Letter(Letter),
    Digit(Digit),
    Dash(StringEq!("-")),
    Dot(StringEq!(".")),
}

#[test]
fn a_hand_written_leaf_parses_checks_traces_and_prints() {
    assert_eq!(Word::parse("héllo").unwrap().print(), "héllo");
    assert_eq!(Word::check_prefix("ab1").unwrap(), 2);
    assert_eq!(
        Word::parse("a1").unwrap_err().to_string(),
        "parse error at byte 2, expected:\n\t- Letter at 1..2 must be a letter"
    );
    let rules: Vec<String> = Word::bnf_rules().iter().map(ToString::to_string).collect();
    assert_eq!(
        rules,
        [
            "Word = Letter { Letter } .",
            "Letter = 'any' ^1 .\n^1: be a letter"
        ]
    );
}

#[test]
fn a_hand_written_leaf_reports_itself_when_skipped() {
    assert_eq!(
        Item::parse("").unwrap_err().to_string(),
        "parse error at byte 0, expected:\n\t- Item\n\t- Letter"
    );
}

/// A digit below five, rejected with a message built from the digit, only
/// in the pass that records it.
#[derive(Debug)]
struct Small(u8);

static BUILT: AtomicUsize = AtomicUsize::new(0);

impl Parse for Small {
    type First = <char as Parse>::First;

    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State<M>,
    ) -> Option<(M::Output<Self>, usize)> {
        let mut state = state.node("Small", pos);
        let (ch, end) = char::build_at(input, pos, state.reborrow())?;
        match ch.to_digit(10) {
            Some(digit) if digit < 5 => Some((M::build(|_| Small(digit as u8)), end)),
            _ => {
                let requirement = if M::RECORD {
                    BUILT.fetch_add(1, Relaxed);
                    format!("be a digit below 5, not {ch:?}")
                } else {
                    String::new()
                };
                state.reject("Small", pos..end, requirement);
                None
            }
        }
    }

    fn fail_at<M: Mode>(pos: usize, mut state: State<M>) -> bool {
        char::fail_at(pos, state.node("Small", pos))
    }

    fn to_bnf() -> Expr {
        Expr::RuleRef("Small".into())
    }

    fn visit_rules(rules: &mut Rules) {
        rules.insert::<Self>("Small", || {
            Expr::side_condition(char::to_bnf(), "be a digit below 5")
        });
    }
}

// Both variants reject "7" with the same built message.
#[derive(Parse, Debug)]
enum Twice {
    A(Small, StringEq!("a")),
    B(Small, StringEq!("b")),
}

#[test]
fn a_rejection_carries_a_message_built_while_parsing() {
    assert_eq!(Small::parse("3").unwrap().0, 3);
    let err = Twice::parse("7").unwrap_err();
    assert_eq!(
        err.to_string(),
        "parse error at byte 1, expected:\n\t- Small at 0..1 must be a digit below 5, not '7'"
    );
    assert_eq!(err.traces().len(), 1);
    // Once per variant, in the recording pass alone.
    assert_eq!(BUILT.load(Relaxed), 2);
}
