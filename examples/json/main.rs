//! JSON parser/printer/EBNF (grammar per json.org), driven from the command line.
//!
//!   cargo run --example json -- bnf
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- parse
//!   echo '{"a": [1, true, null]}' | cargo run --example json -- print
//!   echo '{"a": 1}' | cargo run --example json -- test

use std::io::{self, Read};
use std::process::ExitCode;

use tygr::{Error, Max, Parse, Print, Unit, Value};

mod grammar;

use grammar::Json;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            for rule in Json::bnf_rules() {
                println!("{rule}");
            }
            ExitCode::SUCCESS
        }
        Some("parse") => {
            report(Json::parse::<Value, Max>(&read_stdin()).map(|json| format!("{json:#?}")))
        }
        Some("print") => report(Json::parse::<Value, Max>(&read_stdin()).map(|json| json.print())),
        Some("test") => report(Json::parse::<Unit, Max>(&read_stdin()).map(|()| "ok".to_string())),
        _ => usage(),
    }
}

/// Read all of stdin, minus the trailing JSON whitespace (`echo` appends a newline).
fn read_stdin() -> String {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");
    input.trim_end_matches([' ', '\t', '\n', '\r']).to_owned()
}

/// Print `shown`, or the error, and exit accordingly.
fn report(shown: Result<String, Error>) -> ExitCode {
    match shown {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Print a usage line naming the available verbs and exit with failure.
fn usage() -> ExitCode {
    eprintln!("usage: json <bnf|parse|print|test>");
    eprintln!("  bnf          print the grammar as EBNF");
    eprintln!("  parse        parse stdin, print the AST");
    eprintln!("  print        parse stdin, print it back");
    eprintln!("  test         check stdin is well-formed (exit 0/1)");
    ExitCode::FAILURE
}
