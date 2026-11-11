//! The CLI verbs of `examples/json.rs`.
//! Included as `mod common;`; not a standalone target.

use std::fmt::Debug;
use std::io::{self, Read};
use std::process::ExitCode;

use tygr::{Error, Grammar, Parse};

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

/// `parse`: parse stdin with `R` and print the `Debug` tree.
pub(crate) fn parse<R: Parse + Debug>() -> ExitCode {
    report(R::parse(&read_stdin()).map(|value| format!("{value:#?}")))
}

/// `print`: parse stdin with `R` and print it back.
pub(crate) fn print<R: Grammar>() -> ExitCode {
    report(R::parse(&read_stdin()).map(|value| value.print()))
}

/// `test`: check that stdin is well-formed under `R`; print `ok`, or the error.
pub(crate) fn test<R: Parse>() -> ExitCode {
    report(R::check(&read_stdin()).map(|()| "ok".to_string()))
}

/// Print a usage line naming the available verbs and exit with failure.
pub(crate) fn usage() -> ExitCode {
    eprintln!("usage: json <bnf|parse|print|test>");
    eprintln!("  bnf          print the grammar as EBNF");
    eprintln!("  parse        parse stdin, print the AST");
    eprintln!("  print        parse stdin, print it back");
    eprintln!("  test         check stdin is well-formed (exit 0/1)");
    ExitCode::FAILURE
}
