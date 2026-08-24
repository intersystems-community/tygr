//! Intermediate representation for BNF/EBNF generation with optimization.
//!
//! This module provides an AST for BNF expressions that can be built, optimized,
//! and then formatted. This allows us to:
//! - Simplify repetition patterns based on hidden elements
//! - Detect and simplify enum-as-option patterns
//!
//! Formatted output follows Wirth syntax notation — see the crate-level
//! `## Design` section for the exact notation and its citation.

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
    /// A side-condition beyond the shown syntax: `A ^1` (see [`Expr::side_conditions`]).
    SideCondition(Box<Expr>, &'static str),
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

    /// Create a repetition, dropping any hidden (`Empty`) elements first; an
    /// entirely-hidden inner expression vanishes rather than showing as `{ }`.
    pub fn repetition(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        if expr.is_empty() {
            expr
        } else {
            Expr::Repetition(Box::new(expr))
        }
    }

    /// Create an optional, dropping any hidden (`Empty`) elements first; an
    /// entirely-hidden inner expression vanishes rather than showing as `[ ]`.
    pub fn optional(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        if expr.is_empty() {
            expr
        } else {
            Expr::Optional(Box::new(expr))
        }
    }

    /// Attach a side-condition to `expr`, reported as a numbered marker —
    /// see [`Expr::side_conditions`].
    pub fn side_condition(expr: Expr, text: &'static str) -> Expr {
        Expr::SideCondition(Box::new(expr), text)
    }

    /// This expression's [`SideCondition`](Expr::SideCondition) texts, deduplicated by string
    /// equality, in first-occurrence order — the same order [`format`](
    /// Self::format) numbers their `^1`, `^2`, ... markers. Callers print
    /// these as footnotes after the formatted expression.
    pub fn side_conditions(&self) -> Vec<&'static str> {
        let mut notes = Vec::new();
        self.collect_side_conditions(&mut notes);
        notes
    }

    fn collect_side_conditions(&self, notes: &mut Vec<&'static str>) {
        match self {
            Expr::SideCondition(inner, text) => {
                inner.collect_side_conditions(notes);
                if !notes.contains(text) {
                    notes.push(text);
                }
            }
            Expr::Sequence(exprs) | Expr::Alternation(exprs) => {
                for expr in exprs {
                    expr.collect_side_conditions(notes);
                }
            }
            Expr::Repetition(inner) | Expr::Optional(inner) | Expr::NotFollowedBy(inner) => {
                inner.collect_side_conditions(notes);
            }
            Expr::StringEq(_) | Expr::StringEqCI(_) | Expr::RuleRef(_) | Expr::CharOf(_) => {}
        }
    }

    /// Format this expression as EBNF. Each [`SideCondition`](Expr::SideCondition) renders as a
    /// numbered marker (`^1`, `^2`, ...) rather than its text — see
    /// [`side_conditions`](Self::side_conditions) for the footnotes to print
    /// alongside.
    pub fn format(&self, f: &mut dyn fmt::Write) -> fmt::Result {
        let notes = self.side_conditions();
        self.format_internal(f, Prec::Alternation, &notes)
    }

    fn prec(&self) -> Prec {
        match self {
            _ if self.is_empty() => Prec::Atom,
            Expr::Alternation(_) | Expr::SideCondition(..) => Prec::Alternation,
            Expr::Sequence(_) => Prec::Sequence,
            _ => Prec::Atom,
        }
    }

    /// Wrap in parens when this expression binds looser than the surrounding context.
    fn format_internal(
        &self,
        f: &mut dyn fmt::Write,
        min_prec: Prec,
        notes: &[&'static str],
    ) -> fmt::Result {
        let paren = self.prec() < min_prec;
        if paren {
            write!(f, "( ")?;
        }
        match self {
            Expr::StringEq(s) => write!(f, "\"{}\"", escape_string(s)),
            Expr::StringEqCI(s) => write!(f, "\"{}\"i", escape_string(s)),
            Expr::RuleRef(name) => write!(f, "{name}"),
            Expr::CharOf(name) => write!(f, "'{}'", escape_char_class(name)),
            Expr::Sequence(exprs) => Self::format_list(exprs, " ", Prec::Sequence, f, notes),
            Expr::Alternation(exprs) => {
                Self::format_list(exprs, " | ", Prec::Alternation, f, notes)
            }
            Expr::Repetition(inner) => {
                write!(f, "{{ ")?;
                inner.format_internal(f, Prec::Alternation, notes)?;
                write!(f, " }}")
            }
            Expr::Optional(inner) => {
                write!(f, "[ ")?;
                inner.format_internal(f, Prec::Alternation, notes)?;
                write!(f, " ]")
            }
            Expr::NotFollowedBy(inner) => {
                write!(f, "!")?;
                inner.format_internal(f, Prec::Atom, notes)
            }
            Expr::SideCondition(inner, text) => {
                inner.format_internal(f, Prec::Atom, notes)?;
                let index = notes.iter().position(|note| note == text).unwrap() + 1;
                write!(f, " ^{index}")
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
        notes: &[&'static str],
    ) -> fmt::Result {
        let mut first = true;
        for expr in exprs {
            if !expr.is_empty() {
                if !first {
                    write!(f, "{sep}")?;
                }
                expr.format_internal(f, min_prec, notes)?;
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
