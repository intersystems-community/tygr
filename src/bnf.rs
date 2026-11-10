//! EBNF (Wirth notation): expressions, simplified as they're built, and the
//! named rules a grammar collects.

use crate::Parse;
use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashSet;
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
    /// A character class, formatted as `'name'`.
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
    /// A side-condition beyond the shown syntax: `A ^1` (see
    /// [`Expr::side_conditions`]).
    SideCondition(Box<Expr>, &'static str),
    /// Left out ([`Hidden`](crate::Hidden), `#[tygr(hidden)]`): dropped
    /// from sequences and alternations.
    Hidden,
}

/// Binding tightness, from loosest to tightest.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Prec {
    Alternation,
    Sequence,
    Atom,
}

impl Expr {
    /// The empty [`Sequence`](Expr::Sequence), which consumes nothing.
    pub fn empty() -> Expr {
        Expr::Sequence(vec![])
    }

    /// Is this an empty [`Sequence`](Expr::Sequence) (see [`Expr::empty`])?
    fn is_empty(&self) -> bool {
        matches!(self, Expr::Sequence(exprs) if exprs.is_empty())
    }

    /// Empty or hidden.
    fn is_blank(&self) -> bool {
        self.is_empty() || *self == Expr::Hidden
    }

    /// A flattened sequence without its hidden parts (hidden if all are).
    pub fn sequence(exprs: Vec<Expr>) -> Expr {
        let mut hidden = false;
        let mut parts = Vec::new();
        for expr in exprs {
            match expr {
                Expr::Sequence(inner) => parts.extend(inner),
                Expr::Hidden => hidden = true,
                _ => parts.push(expr),
            }
        }
        match parts.len() {
            0 if hidden => Expr::Hidden,
            1 => parts.pop().unwrap(),
            _ => Expr::Sequence(parts),
        }
    }

    /// A flattened alternation without its hidden alternatives (hidden if
    /// all are); an empty alternative makes the rest optional.
    pub fn alternation(exprs: Vec<Expr>) -> Expr {
        let (mut hidden, mut empty) = (false, false);
        let mut alternatives = Vec::new();
        for expr in exprs {
            match expr {
                Expr::Alternation(inner) => alternatives.extend(inner),
                Expr::Hidden => hidden = true,
                _ if expr.is_empty() => empty = true,
                _ => alternatives.push(expr),
            }
        }
        let alternation = match alternatives.len() {
            0 if hidden && !empty => Expr::Hidden,
            0 => Expr::empty(),
            1 => alternatives.pop().unwrap(),
            _ => Expr::Alternation(alternatives),
        };
        if empty {
            Expr::optional(alternation)
        } else {
            alternation
        }
    }

    /// `{ expr }`, or `expr` if it's empty or hidden.
    pub fn repetition(expr: Expr) -> Expr {
        if expr.is_blank() {
            expr
        } else {
            Expr::Repetition(Box::new(expr))
        }
    }

    /// `[ expr ]`, or `expr` if it's empty or hidden.
    pub fn optional(expr: Expr) -> Expr {
        if expr.is_blank() {
            expr
        } else {
            Expr::Optional(Box::new(expr))
        }
    }

    /// `!expr`, or `expr` if it's hidden.
    pub fn not_followed_by(expr: Expr) -> Expr {
        if expr == Expr::Hidden {
            expr
        } else {
            Expr::NotFollowedBy(Box::new(expr))
        }
    }

    /// `expr` with a footnoted side-condition, or `expr` if it's hidden.
    pub fn side_condition(expr: Expr, text: &'static str) -> Expr {
        if expr == Expr::Hidden {
            expr
        } else {
            Expr::SideCondition(Box::new(expr), text)
        }
    }

    /// The side-condition texts, deduplicated, in the order `Display`
    /// numbers them.
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
            Expr::Token { .. } | Expr::RuleRef(_) | Expr::CharOf(_) | Expr::Hidden => {}
        }
    }

    /// Rename each rule this refers to with `f`.
    fn rename(&mut self, f: &impl Fn(&str) -> String) {
        match self {
            Expr::RuleRef(name) => *name = f(name),
            Expr::Sequence(exprs) | Expr::Alternation(exprs) => {
                for expr in exprs {
                    expr.rename(f);
                }
            }
            Expr::Repetition(inner)
            | Expr::Optional(inner)
            | Expr::NotFollowedBy(inner)
            | Expr::SideCondition(inner, _) => inner.rename(f),
            Expr::Token { .. } | Expr::CharOf(_) | Expr::Hidden => {}
        }
    }

    fn prec(&self) -> Prec {
        match self {
            _ if self.is_blank() => Prec::Atom,
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
                escape(text, '"'),
                if *case_insensitive { "i" } else { "" }
            ),
            Expr::RuleRef(name) => write!(f, "{name}"),
            Expr::CharOf(name) => write!(f, "'{}'", escape(name, '\'')),
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
                if !inner.is_blank() {
                    write!(f, " ")?;
                }
                let index = notes.iter().position(|note| note == text).unwrap() + 1;
                write!(f, "^{index}")
            }
            Expr::Hidden => Ok(()),
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
            if !expr.is_blank() {
                if !first {
                    write!(f, "{sep}")?;
                }
                expr.format_internal(f, min_prec, notes)?;
                first = false;
            }
        }
        Ok(())
    }
}

/// EBNF; side-conditions become `^1`, `^2`, … markers.
impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let notes = self.side_conditions();
        self.format_internal(f, Prec::Alternation, &notes)
    }
}

/// A grammar's BNF, built on demand by its [`to_bnf`](crate::Parse::to_bnf),
/// so recording one costs a function pointer. Equal when the BNF is.
#[derive(Clone, Copy)]
pub struct Lazy(pub fn() -> Expr);

impl PartialEq for Lazy {
    fn eq(&self, other: &Self) -> bool {
        // Function addresses aren't unique, so fall back to the BNF built.
        std::ptr::fn_addr_eq(self.0, other.0) || (self.0)() == (other.0)()
    }
}

impl Eq for Lazy {}

impl fmt::Debug for Lazy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&(self.0)(), f)
    }
}

impl fmt::Display for Lazy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&(self.0)(), f)
    }
}

/// A named rule, `name = expr .`, as [`Parse::bnf_rules`] collects it.
/// `Display` footnotes its side-conditions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// The rule's name: by default, its type's.
    pub name: String,
    /// The rule's definition.
    pub expr: Expr,
}

impl Rule {
    /// Rename this rule, and the rules it refers to, with `f`.
    ///
    /// ```
    /// # use tygr::*;
    /// # char_class!(Digit, "digit", |ch| ch.is_ascii_digit());
    /// #[derive(Parse)]
    /// struct Pair(Number, StringEq!(","), Number);
    /// #[derive(Parse)]
    /// struct Number(StringOf1<Digit>);
    ///
    /// let mut rules = Pair::bnf_rules();
    /// for rule in &mut rules {
    ///     rule.rename(str::to_lowercase);
    /// }
    /// assert_eq!(rules[0].to_string(), "pair = number \",\" number .");
    /// ```
    pub fn rename(&mut self, f: impl Fn(&str) -> String) {
        self.name = f(&self.name);
        self.expr.rename(&f);
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} =", self.name)?;
        if !self.expr.is_blank() {
            write!(f, " {}", self.expr)?;
        }
        write!(f, " .")?;
        for (i, text) in self.expr.side_conditions().into_iter().enumerate() {
            let sep = if i == 0 { "\n\n" } else { "\n" };
            write!(f, "{sep}^{}: {text}", i + 1)?;
        }
        Ok(())
    }
}

/// Collects each type's [`Rule`] once, for [`Parse::visit_rules`].
pub struct Rules {
    seen: HashSet<TypeId>,
    rules: Vec<Rule>,
}

impl Rules {
    /// Add `T`'s rule, `name = expr()`, unless it's added already. Returns
    /// whether it's new, so `T` visits the rules it refers to only once.
    pub fn insert<T: 'static>(&mut self, name: &str, expr: impl FnOnce() -> Expr) -> bool {
        let new = self.seen.insert(TypeId::of::<T>());
        if new {
            self.rules.push(Rule {
                name: name.to_string(),
                expr: expr(),
            });
        }
        new
    }
}

/// `G`'s rules, for [`Parse::bnf_rules`].
pub(crate) fn rules<G: Parse>() -> Vec<Rule> {
    let mut rules = Rules {
        seen: HashSet::new(),
        rules: Vec::new(),
    };
    G::visit_rules(&mut rules);
    rules.rules
}

thread_local! {
    /// The inline grammars being expanded, innermost last, each with
    /// whether it referred to itself.
    static EXPANDING: RefCell<Vec<(TypeId, bool)>> = const { RefCell::new(Vec::new()) };
}

/// `def()`, expanded with `T` marked, and whether it referred to `T`.
fn expand<T: 'static>(def: impl FnOnce() -> Expr) -> (Expr, bool) {
    /// Unmarks `T`, even if `def` panics.
    struct Unmark;
    impl Drop for Unmark {
        fn drop(&mut self) {
            EXPANDING.with_borrow_mut(|stack| {
                stack.pop();
            });
        }
    }
    EXPANDING.with_borrow_mut(|stack| stack.push((TypeId::of::<T>(), false)));
    let _unmark = Unmark;
    let expr = def();
    let recursive = EXPANDING.with_borrow(|stack| stack.last().is_some_and(|&(_, found)| found));
    (expr, recursive)
}

/// An inline grammar `T`'s BNF: `def()` spliced in, or, if it refers to
/// itself, a reference to its own rule.
#[doc(hidden)]
pub fn inline_bnf<T: 'static>(name: &str, def: impl FnOnce() -> Expr) -> Expr {
    let id = TypeId::of::<T>();
    let expanding = EXPANDING.with_borrow_mut(|stack| {
        let marked = stack.iter_mut().find(|(other, _)| *other == id);
        marked.map(|(_, found)| *found = true).is_some()
    });
    if !expanding {
        let (expr, recursive) = expand::<T>(def);
        if !recursive {
            return expr;
        }
    }
    Expr::RuleRef(name.to_string())
}

/// Visit an inline grammar `T`'s rules: its parts', and, if it refers to
/// itself, its own rule, as a named grammar would.
#[doc(hidden)]
pub fn visit_inline<T: 'static>(
    rules: &mut Rules,
    name: &str,
    def: impl FnOnce() -> Expr,
    visit_parts: impl FnOnce(&mut Rules),
) {
    match expand::<T>(def) {
        (expr, true) => {
            if rules.insert::<T>(name, || expr) {
                visit_parts(rules);
            }
        }
        (_, false) => visit_parts(rules),
    }
}

/// `s` escaped for a literal between `quote`s.
pub(crate) fn escape(s: &str, quote: char) -> String {
    s.replace('\\', "\\\\")
        .replace(quote, &format!("\\{quote}"))
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
