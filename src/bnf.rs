//! Intermediate representation for BNF/EBNF generation with optimization.
//!
//! This module provides an AST for BNF expressions that can be built, optimized,
//! and then formatted. This allows us to:
//! - Merge consecutive `Lit` tokens into multi-character strings
//! - Combine case-insensitive literal sequences
//! - Simplify repetition patterns based on hidden elements
//! - Detect and simplify enum-as-option patterns

use std::fmt;

/// Intermediate representation of a BNF/EBNF expression.
/// All instances are maintained in optimized form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Literal(String),
    LiteralCI(String),
    RuleRef(String),
    CharOf(String),
    Sequence(Vec<Expr>),
    Alternation(Vec<Expr>),
    Repetition(Box<Expr>),
    Optional(Box<Expr>),
    NotFollowedBy(Box<Expr>),
    Empty,
}

/// Binding tightness, from loosest to tightest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Alternation,
    Sequence,
    Atom,
}

impl Expr {
    /// Create a sequence, automatically flattening and merging consecutive literals.
    pub fn sequence(mut exprs: Vec<Expr>) -> Expr {
        // flatten nested sequences
        exprs = exprs
            .into_iter()
            .flat_map(|expr| match expr {
                Expr::Sequence(inner) => inner,
                _ => vec![expr],
            })
            .collect();
        // Merge consecutive literals
        exprs = Self::merge_literals(exprs);
        match exprs.len() {
            1 => exprs.into_iter().next().unwrap(),
            _ => Expr::Sequence(exprs),
        }
    }

    /// Create an alternation, automatically flattening.
    pub fn alternation(mut exprs: Vec<Expr>) -> Expr {
        exprs = exprs
            .into_iter()
            .flat_map(|expr| match expr {
                Expr::Alternation(inner) => inner,
                _ => vec![expr],
            })
            .collect();

        match exprs.len() {
            1 => exprs.into_iter().next().unwrap(),
            _ => Expr::Alternation(exprs),
        }
    }

    /// Merge consecutive Literal and LiteralCI tokens in a sequence.
    fn merge_literals(exprs: Vec<Expr>) -> Vec<Expr> {
        let mut result = Vec::new();

        for expr in exprs {
            match (result.last_mut(), expr) {
                (Some(Expr::Literal(last)), Expr::Literal(s)) => {
                    last.push_str(&s);
                }
                (Some(Expr::LiteralCI(last)), Expr::LiteralCI(s)) => {
                    last.push_str(&s);
                }
                (_, expr) => {
                    result.push(expr);
                }
            }
        }
        result
    }

    pub fn repetition(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        Expr::Repetition(Box::new(expr))
    }

    pub fn optional(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        Expr::Optional(Box::new(expr))
    }

    /// Format this expression as EBNF.
    pub fn format(&self, f: &mut dyn fmt::Write) -> fmt::Result {
        self.format_internal(f, Prec::Alternation)
    }

    fn prec(&self) -> Prec {
        match self {
            Expr::Alternation(_) => Prec::Alternation,
            Expr::Sequence(_) => Prec::Sequence,
            _ => Prec::Atom,
        }
    }

    /// Wrap in parens when this expression binds looser than the surrounding context.
    fn format_internal(&self, f: &mut dyn fmt::Write, min_prec: Prec) -> fmt::Result {
        let paren = self.prec() < min_prec;
        if paren {
            write!(f, "( ")?;
        }
        match self {
            Expr::Empty => Ok(()),
            Expr::Literal(s) => write!(f, "\"{}\"", escape_string(s)),
            Expr::LiteralCI(s) => write!(f, "\"{}\"i", escape_string(s)),
            Expr::RuleRef(name) => write!(f, "{name}"),
            Expr::CharOf(name) => write!(f, "'{}'", escape_char_class(name)),
            Expr::Sequence(exprs) => Self::format_list(exprs, " ", Prec::Sequence, f),
            Expr::Alternation(exprs) => Self::format_list(exprs, " | ", Prec::Alternation, f),
            Expr::Repetition(inner) => {
                write!(f, "{{ ")?;
                inner.format_internal(f, Prec::Alternation)?;
                write!(f, " }}")
            }
            Expr::Optional(inner) => {
                write!(f, "[ ")?;
                inner.format_internal(f, Prec::Alternation)?;
                write!(f, " ]")
            }
            Expr::NotFollowedBy(inner) => {
                write!(f, "!")?;
                inner.format_internal(f, Prec::Atom)
            }
        }?;
        if paren {
            write!(f, " )")?;
        }
        Ok(())
    }

    fn format_list(
        exprs: &[Expr],
        sep: &str,
        min_prec: Prec,
        f: &mut dyn fmt::Write,
    ) -> fmt::Result {
        let mut first = true;
        for expr in exprs {
            if *expr != Expr::Empty {
                if !first {
                    write!(f, "{sep}")?;
                }
                expr.format_internal(f, min_prec)?;
                first = false;
            }
        }
        Ok(())
    }

    fn remove_empties(self) -> Expr {
        if let Expr::Sequence(mut exprs) = self {
            exprs.retain(|expr| *expr != Expr::Empty);
            match exprs.len() {
                0 => Expr::Empty,
                1 => exprs.pop().unwrap(),
                _ => Expr::Sequence(exprs),
            }
        } else {
            self
        }
    }
}

/// Escape special characters in a string literal.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn escape_char_class(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
