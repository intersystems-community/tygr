use std::process::ExitCode;

use tygr::*;

#[path = "common/mod.rs"]
mod common;

char_class!(OtherChar, "other char", |ch| !matches!(ch, '[' | ']'));

type Main = VecSep<Node, StringEq!("~")>;

#[derive(Grammar, Debug)]
enum Node {
    Subsegment(Box<Subsegment>),
    Group {
        name: Option<StringOf1<OtherChar>>,
        head: Prefix<StringEq!("~"), Box<Subsegment>>,
        tail: Vec<Prefix<StringEq!("~"), Box<Subsegment>>>,
    },
}

#[derive(Grammar, Debug)]
enum Subsegment {
    G(Wrap<StringEq!("("), Node, StringEq!(")")>),
    O(Wrap<StringEq!("["), Node, StringEq!("]")>),
    R(Wrap<StringEq!("{"), Node, StringEq!("}")>),
    U(Wrap<StringEq!("<"), (Node, Vec<Prefix<StringEq!("|"), Node>>), StringEq!(">")>),
    Segment(StringOf1<OtherChar>),
}

pub struct CanonicalNode {
    optional: bool,
    repeating: bool,
    kind: Box<CanonicalNodeKind>,
}

enum CanonicalNodeKind {
    Segment(String),
    Group {
        name: Option<String>,
        head: CanonicalNode,
        tail: Vec<CanonicalNode>,
    },
    Union {
        name: Option<String>,
        head: CanonicalNode,
        tail: Vec<CanonicalNode>,
    },
}

// Conversion

impl CanonicalNode {
    fn group(head: CanonicalNode, tail: Vec<CanonicalNode>) -> CanonicalNode {
        if tail.is_empty() {
            head
        } else {
            CanonicalNodeKind::Group {
                name: None,
                head,
                tail,
            }
            .into()
        }
    }

    fn union(head: CanonicalNode, tail: Vec<CanonicalNode>) -> CanonicalNode {
        if tail.is_empty() {
            head
        } else {
            CanonicalNodeKind::Union {
                name: None,
                head,
                tail,
            }
            .into()
        }
    }

    fn segment(name: String) -> Self {
        CanonicalNodeKind::Segment(name).into()
    }

    fn optional(self) -> Self {
        Self {
            optional: true,
            ..self
        }
    }

    fn repeating(self) -> Self {
        Self {
            repeating: true,
            ..self
        }
    }

    fn name(mut self, name: String) -> Self {
        self.kind = Box::new(self.kind.with_name(name));
        self
    }
}

impl CanonicalNodeKind {
    fn as_group(self) -> Self {
        Self::Group {
            name: None,
            head: Self::Group {
                name: None,
                head: self.into(),
                tail: vec![],
            }
            .into(),
            tail: vec![],
        }
    }

    fn with_name(self, name: String) -> Self {
        match self {
            CanonicalNodeKind::Group {
                name: _,
                head,
                tail,
            } => Self::Group {
                name: Some(name),
                head,
                tail,
            },
            CanonicalNodeKind::Union {
                name: _,
                head,
                tail,
            } => Self::Union {
                name: Some(name),
                head,
                tail,
            },
            other => other.as_group(),
        }
    }
}

impl From<CanonicalNodeKind> for CanonicalNode {
    fn from(kind: CanonicalNodeKind) -> Self {
        Self {
            optional: false,
            repeating: false,
            kind: Box::new(kind),
        }
    }
}

impl From<Node> for CanonicalNode {
    fn from(node: Node) -> Self {
        match node {
            Node::Subsegment(subsegment) => (*subsegment).into(),
            Node::Group { name, head, tail } => {
                let node = Self::group(
                    (*head.into_inner()).into(),
                    tail.into_iter()
                        .map(|tail| (*tail.into_inner()).into())
                        .collect(),
                );
                if let Some(name) = name {
                    node.name(name.into_inner())
                } else {
                    node
                }
            }
        }
    }
}

impl From<Subsegment> for CanonicalNode {
    fn from(node: Subsegment) -> Self {
        match node {
            Subsegment::G(node) => Self::from(node.into_inner()),
            Subsegment::O(node) => Self::from(node.into_inner()).optional(),
            Subsegment::R(node) => Self::from(node.into_inner()).repeating(),
            Subsegment::U(nodes) => {
                let (head, tail) = nodes.into_inner();
                Self::union(
                    head.into(),
                    tail.into_iter()
                        .map(|node| node.into_inner().into())
                        .collect(),
                )
            }
            Subsegment::Segment(segment) => Self::segment(segment.into_inner()),
        }
    }
}

impl From<CanonicalNode> for Node {
    fn from(node: CanonicalNode) -> Self {
        if node.optional {
            Self::Subsegment(Box::new(Subsegment::O(Wrap {
                before: Default::default(),
                wrapped: Self::from(CanonicalNode {
                    optional: false,
                    ..node
                }),
                after: Default::default(),
            })))
        } else if node.repeating {
            Self::Subsegment(Box::new(Subsegment::R(Wrap {
                before: Default::default(),
                wrapped: Self::from(CanonicalNode {
                    repeating: false,
                    ..node
                }),
                after: Default::default(),
            })))
        } else {
            match (node.name, node.kind) {}
        }
    }
}

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("bnf") => {
            println!("{}", bnf_rules![Node, Subsegment]);
            ExitCode::SUCCESS
        }
        Some("parse") => common::parse::<Main>(),
        Some("test") => common::test::<Main>(),
        _ => common::usage("json"),
    }
}
