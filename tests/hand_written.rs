//! A leaf grammar implemented by hand, through the public API.

use tygr::bnf::{Expr, Rules};
use tygr::*;

#[derive(Debug)]
struct Letter(char);

impl Parse for Letter {
    type First = <char as Parse>::First;

    fn parse_at<M: Mode>(
        input: &str,
        pos: usize,
        mut state: State,
    ) -> Option<(M::Output<Self>, usize)> {
        let mut state = state.node("Letter", pos);
        let (ch, end) = char::parse_at::<Build>(input, pos, state.reborrow())?;
        if !ch.is_alphabetic() {
            state.reject(Some("Letter"), pos..end, "be a letter");
            return None;
        }
        Some((M::build(|_| Letter(ch)), end))
    }

    fn fail_at(pos: usize, mut state: State) -> bool {
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
    #[cfg(feature = "trace_one_node")]
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
    let err = Item::parse("").unwrap_err();
    if cfg!(feature = "trace_one_node") {
        assert_eq!(
            err.to_string(),
            "parse error at byte 0, expected:\n\t- Item\n\t- Letter"
        );
    }
}
