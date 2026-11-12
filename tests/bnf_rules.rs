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
    struct HiddenNumber(Number);
    assert_eq!(
        printed(Inline::bnf_rules()),
        ["Number = 'digit' { 'digit' } ."]
    );
    assert!(HiddenNumber::bnf_rules().is_empty());
}

#[test]
fn recursive_inline_grammars_get_a_rule_of_their_own() {
    #[derive(Parse)]
    #[tygr(inline)]
    struct Nest(StringEq!("("), Option<Box<Nest>>, StringEq!(")"));
    #[derive(Parse)]
    struct Group(Nest, StringEq!(";"));
    assert_eq!(printed(Nest::bnf_rules()), [r#"Nest = "(" [ Nest ] ")" ."#]);
    assert_eq!(
        printed(Group::bnf_rules()),
        [r#"Group = Nest ";" ."#, r#"Nest = "(" [ Nest ] ")" ."#]
    );
    // Through another inline type, which must not claim the cycle as its own.
    #[derive(Parse)]
    #[tygr(inline)]
    struct NestW(Wrap<StringEq!("("), Option<Box<NestW>>, StringEq!(")")>);
    assert_eq!(
        printed(NestW::bnf_rules()),
        [r#"NestW = "(" [ NestW ] ")" ."#]
    );
}

#[test]
fn mutually_recursive_inline_grammars_share_one_rule() {
    #[derive(Parse)]
    #[tygr(inline)]
    struct A(B);
    #[derive(Parse)]
    #[tygr(inline)]
    struct B(StringEq!("("), Option<Box<A>>, StringEq!(")"));
    #[derive(Parse)]
    struct Group(A, StringEq!(";"));
    assert_eq!(
        printed(Group::bnf_rules()),
        [r#"Group = A ";" ."#, r#"A = "(" [ A ] ")" ."#]
    );
}

#[test]
fn empty_validated_operand_stays_visible_under_its_footnote() {
    #[derive(Parse)]
    #[tygr(inline, validated)]
    struct Marker;
    impl Validate for Marker {
        const REQUIREMENT: &'static str = "be allowed here";
        fn validate(&self) -> bool {
            true
        }
    }
    #[derive(Parse)]
    struct UsesEmptyValidated(StringOf1<Digit>, Marker, StringEq!(","));
    assert_eq!(
        printed(UsesEmptyValidated::bnf_rules()),
        ["UsesEmptyValidated = 'digit' { 'digit' } ( ) ^1 \",\" .\n^1: be allowed here"]
    );
}

#[test]
fn an_empty_enum_bnf_is_footnoted_never() {
    #[derive(Parse)]
    enum Never {}
    assert_eq!(
        printed(Never::bnf_rules()),
        ["Never = ( ) ^1 .\n^1: never matches"]
    );
}

#[test]
fn looser_operands_are_parenthesized() {
    #[derive(Parse)]
    #[tygr(inline)]
    enum AB {
        A(StringEq!("a")),
        B(StringEq!("b")),
    }
    #[derive(Parse)]
    struct Wrapped(AB, NotFollowedBy<(StringEq!("x"), StringEq!("y"))>);
    assert_eq!(
        printed(Wrapped::bnf_rules()),
        [r#"Wrapped = ( "a" | "b" ) !( "x" "y" ) ."#]
    );
}
