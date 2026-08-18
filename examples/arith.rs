//! The example from README.md. Kept in sync — if you change one, change both.

use tygr::*;

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Expr(pub Expr1, pub Vec<(Wrap<Ws, Op1, Ws>, Expr1)>);

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Op1 {
    Add(StringEq!("+")),
    Sub(StringEq!("-")),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Expr1(pub Expr2, pub Vec<(Wrap<Ws, Op2, Ws>, Expr2)>);

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Op2 {
    Mul(StringEq!("*")),
    Div(StringEq!("/")),
}

#[derive(Grammar, Debug, PartialEq, Eq)]
pub enum Expr2 {
    Paren(Wrap<(StringEq!("("), Ws), Box<Expr>, (Ws, StringEq!(")"))>),
    Number(Int),
}

char_class!(pub IsDigit, "digit", |ch| ch.is_ascii_digit());

#[derive(Grammar, Debug, PartialEq, Eq)]
pub struct Int(pub StringOf1<IsDigit>); // StringOf1: one or more digits

char_class!(pub IsSpace, "space", |ch| ch.is_ascii_whitespace());

#[derive(Grammar, Debug, PartialEq, Eq)]
#[grammar(hidden)] // parsed & printed, but omitted from the BNF
pub struct Ws(pub StringOf<IsSpace>); // StringOf: zero or more spaces

#[path = "common/mod.rs"]
mod common;

fn main() -> std::process::ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            println!("{}", bnf_rules![Expr, Op1, Expr1, Op2, Expr2, Int]);
            std::process::ExitCode::SUCCESS
        }
        Some("parse") => common::parse::<Expr>(),
        Some("test") => common::test::<Expr>(),
        _ => common::usage("arith"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let e: Expr = Expr::parse("1 + 2 * 3").unwrap();
        let ws = || Ws::parse(" ").unwrap();
        assert_eq!(
            e,
            Expr(
                Expr1(Expr2::parse("1").unwrap(), vec![]),
                vec![(
                    Wrap::new(ws(), Op1::Add(Default::default()), ws()),
                    Expr1(
                        Expr2::parse("2").unwrap(),
                        vec![(
                            Wrap::new(ws(), Op2::Mul(Default::default()), ws()),
                            Expr2::parse("3").unwrap()
                        )]
                    )
                )]
            )
        );
        assert_eq!(e.print(), "1 + 2 * 3");
    }

    #[test]
    fn generates_bnf() {
        let bnf = bnf_rules![Expr, Op1, Expr1, Op2, Expr2, Int].to_string();
        assert_eq!(
            bnf,
            "\
expr = expr1 { op1 expr1 } ;
op1 = \"+\" | \"-\" ;
expr1 = expr2 { op2 expr2 } ;
op2 = \"*\" | \"/\" ;
expr2 = \"(\" expr \")\" | int ;
int = 'digit' { 'digit' } ;"
        );
    }
}
