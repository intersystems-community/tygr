//! `Parse::bnf_rules`: each type's rule once, depth first.

use tygr::*;

char_class!(Digit, "digit", |ch| ch.is_ascii_digit());

fn printed(rules: Vec<bnf::Rule>) -> Vec<String> {
    rules.iter().map(ToString::to_string).collect()
}

#[derive(Parse)]
struct Sum(Term, Vec<(StringEq!("+"), Term)>);

#[derive(Parse)]
enum Term {
    Paren(StringEq!("("), Box<Sum>, StringEq!(")")),
    Number(Number),
}

#[derive(Parse)]
struct Number(StringOf1<Digit>);

#[test]
fn each_rule_once_depth_first() {
    assert_eq!(
        printed(Sum::bnf_rules()),
        [
            r#"Sum = Term { "+" Term } ."#,
            r#"Term = "(" Sum ")" | Number ."#,
            "Number = 'digit' { 'digit' } .",
        ]
    );
}

mod a {
    #[derive(tygr::Parse)]
    pub(super) struct Name(tygr::StringEq!("a"));
}

mod b {
    #[derive(tygr::Parse)]
    pub(super) struct Name(tygr::StringEq!("b"));
}

#[derive(Parse)]
struct Both(a::Name, b::Name);

#[test]
fn types_named_alike_are_separate_rules() {
    assert_eq!(
        printed(Both::bnf_rules()),
        ["Both = Name Name .", r#"Name = "a" ."#, r#"Name = "b" ."#]
    );
}

#[test]
fn renaming_covers_references() {
    let mut rules = Sum::bnf_rules();
    for rule in &mut rules {
        rule.rename(str::to_uppercase);
    }
    assert_eq!(
        printed(rules),
        [
            r#"SUM = TERM { "+" TERM } ."#,
            r#"TERM = "(" SUM ")" | NUMBER ."#,
            "NUMBER = 'digit' { 'digit' } .",
        ]
    );
}

#[test]
fn inline_or_hidden_roots_have_no_rule_of_their_own() {
    #[derive(Parse)]
    #[tygr(inline)]
    struct Inline(Number);
    #[derive(Parse)]
    #[tygr(hidden)]
    struct Spaces(Number);
    assert_eq!(
        printed(Inline::bnf_rules()),
        ["Number = 'digit' { 'digit' } ."]
    );
    assert!(Spaces::bnf_rules().is_empty());
}
