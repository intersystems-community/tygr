//! Shared CLI dispatch for the runnable grammar examples.
//!
//! `#[path]`-included by each example. Not a standalone target.

use std::fmt::Debug;
use std::io::{self, Read};
use std::process::ExitCode;

use tygr::Grammar;

/// Read all of stdin into a `String`.
fn read_stdin() -> String {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");
    input
}

/// `parse`: parse stdin with `R` and print the `Debug` tree.
pub fn parse<R: Grammar + Debug>() -> ExitCode {
    let input = read_stdin();
    match R::parse(input.trim_end()) {
        Ok(value) => {
            println!("{value:#?}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("parse error{err}");
            ExitCode::FAILURE
        }
    }
}

/// `test`: check that stdin is well-formed under `R`; print `ok` / `error`.
pub fn test<R: Grammar>() -> ExitCode {
    let input = read_stdin();
    match R::scan(input.trim_end()) {
        Ok(()) => {
            println!("ok");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error{err}");
            ExitCode::FAILURE
        }
    }
}

/// Print a usage line naming the available verbs and exit with failure.
pub fn usage(name: &str) -> ExitCode {
    eprintln!("usage: {name} <bnf|parse|test>");
    eprintln!("  bnf          print the grammar as EBNF");
    eprintln!("  parse        parse stdin, print the AST");
    eprintln!("  test         check stdin is well-formed (exit 0/1)");
    ExitCode::FAILURE
}
