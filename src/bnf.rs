//! Intermediate representation for BNF/EBNF generation with optimization.
//!
//! This module provides an AST for BNF expressions that can be built, optimized,
//! and then formatted. This allows us to:
//! - Simplify repetition patterns based on hidden elements
//! - Detect and simplify enum-as-option patterns

use std::fmt;

/// Intermediate representation of a BNF/EBNF expression.
/// All instances are maintained in optimized form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A case-sensitive literal, formatted as `"text"`.
    StringEq(String),
    /// A case-insensitive literal, formatted as `"text"i`.
    StringEqCI(String),
    /// A reference to another named rule.
    RuleRef(String),
    /// A character class, formatted as `'label'`.
    CharOf(String),
    /// Concatenation: `A B C`.
    Sequence(Vec<Expr>),
    /// Alternation: `A | B | C`.
    Alternation(Vec<Expr>),
    /// Zero or more repetitions: `{ A }`.
    Repetition(Box<Expr>),
    /// Zero or one: `[ A ]`.
    Optional(Box<Expr>),
    /// Negative lookahead: `!A`.
    NotFollowedBy(Box<Expr>),
}

/// Binding tightness, from loosest to tightest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Alternation,
    Sequence,
    Atom,
}

impl Expr {
    /// Matches nothing; omitted from formatted output. Represented as an
    /// empty [`Sequence`](Expr::Sequence) rather than its own variant, since
    /// `format_list` already renders one as nothing.
    pub fn empty() -> Expr {
        Expr::Sequence(vec![])
    }

    /// Is this an empty [`Sequence`](Expr::Sequence) (see [`Expr::empty`])?
    fn is_empty(&self) -> bool {
        matches!(self, Expr::Sequence(exprs) if exprs.is_empty())
    }

    /// Create a sequence, automatically flattening nested sequences.
    pub fn sequence(mut exprs: Vec<Expr>) -> Expr {
        // flatten nested sequences
        exprs = exprs
            .into_iter()
            .flat_map(|expr| match expr {
                Expr::Sequence(inner) => inner,
                _ => vec![expr],
            })
            .collect();
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

    /// Create a repetition, dropping any hidden (`Empty`) elements first.
    pub fn repetition(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        Expr::Repetition(Box::new(expr))
    }

    /// Create an optional, dropping any hidden (`Empty`) elements first.
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
            _ if self.is_empty() => Prec::Atom,
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
            Expr::StringEq(s) => write!(f, "\"{}\"", escape_string(s)),
            Expr::StringEqCI(s) => write!(f, "\"{}\"i", escape_string(s)),
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
            if !expr.is_empty() {
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
            exprs.retain(|expr| !expr.is_empty());
            match exprs.len() {
                0 => Expr::empty(),
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
