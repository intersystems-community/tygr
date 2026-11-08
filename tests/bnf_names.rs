//! `lower_bnf_name` / `upper_bnf_name` rename default rule names only.

use tygr::*;

/// `G`'s own rule, as printed.
fn rule<G: Parse>() -> String {
    G::bnf_rules()[0].to_string()
}

char_class!(Digit, |ch| ch.is_ascii_digit());

#[derive(Grammar)]
struct NumberList(StringOf1<Digit>, Vec<(StringEq!(","), Pinned)>);

#[derive(Grammar)]
#[tygr(name = "Pinned")]
struct Pinned(StringOf1<Digit>);

/// `name` as the enabled feature renames a default name.
fn default(name: &str) -> String {
    if cfg!(feature = "lower_bnf_name") && !cfg!(feature = "upper_bnf_name") {
        name.to_ascii_lowercase()
    } else if cfg!(feature = "upper_bnf_name") && !cfg!(feature = "lower_bnf_name") {
        name.to_ascii_uppercase()
    } else {
        name.to_string()
    }
}

#[test]
fn default_names_follow_the_feature_explicit_names_do_not() {
    let (list, digit) = (default("NumberList"), default("Digit"));
    assert_eq!(
        rule::<NumberList>(),
        format!("{list} = '{digit}' {{ '{digit}' }} {{ \",\" Pinned }} .")
    );
    assert_eq!(
        rule::<Pinned>(),
        format!("Pinned = '{digit}' {{ '{digit}' }} .")
    );
}
