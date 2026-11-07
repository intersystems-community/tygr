//! JSON parser/printer/EBNF (grammar per json.org), driven from the command line.
//!
//!   cargo run --example json -- bnf
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- parse
//!   echo '{"a": 1}' | cargo run --example json -- test

use std::process::ExitCode;

use tygr::*;

#[path = "common/mod.rs"]
mod common;
#[path = "common/json.rs"]
mod json;

use json::*;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            println!(
                "{}",
                bnf_rules![
                    Json, Element, Object, Members, Member, Array, Elements, String, Characters,
                    Character, Escape, Hex, Number, Integer, Digits, Digit, Fraction, Exponent,
                    Sign, Ws,
                ]
            );
            ExitCode::SUCCESS
        }
        Some("parse") => common::parse::<Json>(),
        Some("test") => common::test::<Json>(),
        _ => common::usage("json"),
    }
}
