//! JSON parser/printer/EBNF (grammar per json.org), driven from the command line.
//!
//!   cargo run --example json -- bnf
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- parse
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- print
//!   echo '{"a": 1}' | cargo run --example json -- test

use std::process::ExitCode;

use tygr::Parse;

mod common;
#[path = "common/json.rs"]
mod json;

use json::Json;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            for rule in Json::bnf_rules() {
                println!("{rule}");
            }
            ExitCode::SUCCESS
        }
        Some("parse") => common::parse::<Json>(),
        Some("print") => common::print::<Json>(),
        Some("test") => common::test::<Json>(),
        _ => common::usage("json"),
    }
}
