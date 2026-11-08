//! Shared CLI dispatch for the runnable grammar examples.
//!
//! `#[path]`-included by each example. Not a standalone target.

use std::fmt::Debug;
use std::io::{self, Read};
use std::process::ExitCode;

use tygr::{Grammar, Parse};

/// Read all of stdin into a `String`.
fn read_stdin() -> String {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");
    input
}

/// Parse stdin with `R`, then print `show` of the value, or the error.
fn parse_then<R: Parse>(show: impl FnOnce(R) -> String) -> ExitCode {
    match R::parse(read_stdin().trim_end()) {
        Ok(value) => {
            println!("{}", show(value));
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
    parse_then(|value: R| format!("{value:#?}"))
}

/// `print`: parse stdin with `R` and print it back.
pub(crate) fn print<R: Grammar>() -> ExitCode {
    parse_then(|value: R| value.print())
}

/// `test`: check that stdin is well-formed under `R`; print `ok` / `error`.
pub(crate) fn test<R: Parse>() -> ExitCode {
    let input = read_stdin();
    match R::check(input.trim_end()) {
        Ok(()) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        }
    }
}

/// Print a usage line naming the available verbs and exit with failure.
pub(crate) fn usage(name: &str) -> ExitCode {
    eprintln!("usage: {name} <bnf|parse|print|test>");
    eprintln!("  bnf          print the grammar as EBNF");
    eprintln!("  parse        parse stdin, print the AST");
    eprintln!("  print        parse stdin, print it back");
    eprintln!("  test         check stdin is well-formed (exit 0/1)");
    ExitCode::FAILURE
}
