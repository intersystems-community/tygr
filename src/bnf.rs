//! EBNF expressions (Wirth notation), simplified as they're built.

use std::fmt;

/// An EBNF expression, kept simplified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    /// A token, formatted as `"text"` or `"text"i`.
    Token {
        /// The literal.
        text: String,
        /// Also matches other cases (`"text"i`).
        case_insensitive: bool,
    },
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
    /// Matches nothing and prints as nothing (an empty
    /// [`Sequence`](Expr::Sequence)).
    pub fn empty() -> Expr {
        Expr::Sequence(vec![])
    }

    /// Is this an empty [`Sequence`](Expr::Sequence) (see [`Expr::empty`])?
    fn is_empty(&self) -> bool {
        matches!(self, Expr::Sequence(exprs) if exprs.is_empty())
    }

    /// A flattened sequence.
    pub fn sequence(mut exprs: Vec<Expr>) -> Expr {
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

    /// A flattened alternation.
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

    /// `{ expr }`, or nothing if `expr` is empty.
    pub fn repetition(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        if expr.is_empty() {
            expr
        } else {
            Expr::Repetition(Box::new(expr))
        }
    }

    /// `[ expr ]`, or nothing if `expr` is empty.
    pub fn optional(expr: Expr) -> Expr {
        let expr = expr.remove_empties();
        if expr.is_empty() {
            expr
        } else {
            Expr::Optional(Box::new(expr))
        }
    }

    /// `expr` with a footnoted side-condition.
    pub fn side_condition(expr: Expr, text: &'static str) -> Expr {
        Expr::SideCondition(Box::new(expr), text)
    }

    /// The side-condition texts, deduplicated, in the order
    /// [`format`](Self::format) numbers them.
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
            Expr::Token { .. } | Expr::RuleRef(_) | Expr::CharOf(_) => {}
        }
    }

    /// Write as EBNF; side-conditions become `^1`, `^2`, … markers.
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

    /// Parenthesized when it binds looser than its context.
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
            Expr::Token {
                text,
                case_insensitive,
            } => write!(
                f,
                "\"{}\"{}",
                escape_string(text),
                if *case_insensitive { "i" } else { "" }
            ),
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
pub(crate) fn escape_string(s: &str) -> String {
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
