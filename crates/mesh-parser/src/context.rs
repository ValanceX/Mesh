//! Completion context: what kind of place the cursor is in (outline D4).
//!
//! **Editor-only**, like [`crate::recover`]: never part of [`crate::parse`]
//! or of a compile. Knowing *where* the cursor is is syntax, so it's
//! answered here, from the Tree-sitter tree; *what* may be written there
//! is decided elsewhere, from the manifest and analysis.
//!
//! While completion is wanted the file is almost always broken, and the
//! cursor is often not inside the node that says what the place is: after
//! `<avatar ` it's in the next text node, and the `.` of `user.` is an
//! `ERROR` beside `user`. So the context is read from the **tokens before
//! the cursor**, and the kinds of their parents. Every rule matches a
//! shape observed on real recovery trees (the Pass 4 plan's table, K1–K6);
//! anything else is no context, and completion then offers nothing.

use mesh_syntax::{Expression, Span};
use tree_sitter::Node;

use crate::{lower_member_access, lower_reference, nesting, span_of, text_of};

/// Where the cursor is, and the partial name it would replace.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Completion {
    pub context: Context,
    /// The partial name under the cursor, which a completion replaces;
    /// an empty span at the cursor when there is none.
    pub replace: Span,
}

/// What kind of name may be written at the cursor.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Context {
    /// A tag name, after `<`.
    TagName,
    /// An attribute name, or an `on.` event binding, in the opening tag of
    /// an element named `tag`.
    AttributeName { tag: String },
    /// An event name after `on.`, in the opening tag of `tag`.
    EventName { tag: String },
    /// The start of an `on.` handler: its whole expression.
    Handler,
    /// Anywhere else an expression may start, including a handler
    /// command's arguments.
    Value,
    /// A member of `object`, after its `.`.
    Member {
        object: Expression,
        object_span: Span,
    },
}

/// The completion context at byte `offset` of `source`, or `None` if the
/// cursor isn't in a place a name completes. Total: an offset past the
/// end is the end, and one inside a character is that character's start.
pub fn context_at(source: &str, offset: usize) -> Option<Completion> {
    let offset = floor_char_boundary(source, offset);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_mprx::language()).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    // K3 lowers an expression recursively; nothing recursive runs on a
    // tree deeper than the limit.
    if nesting::check(root).is_some() {
        return None;
    }
    let tokens = tokens_before(root, offset);

    // The partial name under the cursor, and the token before it.
    let (replace, before) = match tokens.last() {
        Some(last)
            if matches!(last.kind(), "identifier" | "tag_name")
                && last.start_byte() < offset
                && offset <= last.end_byte() =>
        {
            (span_of(*last), tokens.len() - 1)
        }
        _ => (
            Span {
                start_byte: offset,
                end_byte: offset,
            },
            tokens.len(),
        ),
    };
    let previous = before.checked_sub(1).map(|index| (index, tokens[index]));

    // K2: `on.` right before the name.
    if let Some(completion) = event_name(source, offset, &tokens) {
        return Some(completion);
    }
    let (index, p) = previous?;
    let context = match p.kind() {
        // K1; a `<` that is an operator is K6.
        "<" if p
            .parent()
            .is_some_and(|parent| parent.kind() == "binary_expression") =>
        {
            value_after(p)
        }
        "<" => Some(Context::TagName),
        // K3
        "." => member(p, index, &tokens, source),
        // K5, K6
        "{" => match p.parent() {
            Some(block) if block.kind() == "expression_block" => {
                let handler = block.parent().is_some_and(|binding| {
                    binding.kind() == "event_binding"
                        && binding
                            .child_by_field_name("handler")
                            .is_some_and(|handler| handler.id() == block.id())
                });
                Some(if handler {
                    Context::Handler
                } else {
                    Context::Value
                })
            }
            _ => None,
        },
        "(" | "," | "[" | "?" | ":" | "!" | "-" | "+" | "*" | "/" | "%" | "<=" | ">=" | "=="
        | "!=" | "&&" | "||" | ">" => value_after(p),
        _ => None,
    };
    // K4, where nothing above matched.
    let context = context.or_else(|| attribute_name(p, index, &tokens, source));
    Some(Completion {
        context: context?,
        replace,
    })
}

/// Every token (leaf) that starts before `offset`, in source order,
/// leaving out `MISSING` and zero-width ones and text, which is never
/// syntax. Iterative, so a deep tree can't exhaust the stack.
fn tokens_before(root: Node, offset: usize) -> Vec<Node> {
    let mut tokens = Vec::new();
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        if node.start_byte() >= offset && node.start_byte() != node.end_byte() {
            continue;
        }
        if node.child_count() == 0 {
            let keep = !node.is_missing()
                && node.start_byte() < node.end_byte()
                && node.start_byte() < offset
                && node.kind() != "text";
            if keep {
                tokens.push(node);
            }
            continue;
        }
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }
    tokens
}

/// K3: after `.`, the `reference` or `member_access` that ends exactly at
/// the dot, lowered.
fn member(dot: Node, index: usize, tokens: &[Node], source: &str) -> Option<Context> {
    let before = *tokens.get(index.checked_sub(1)?)?;
    if before.end_byte() != dot.start_byte() {
        return None;
    }
    let mut object = None;
    let mut node = before;
    while let Some(parent) = node.parent() {
        if parent.end_byte() != dot.start_byte() {
            break;
        }
        if matches!(parent.kind(), "reference" | "member_access") {
            object = Some(parent);
        }
        node = parent;
    }
    let object = object.filter(|object| !object.has_error())?;
    let expression = match object.kind() {
        "reference" => Expression::Reference(lower_reference(object, source)),
        _ => Expression::MemberAccess(lower_member_access(object, source)),
    };
    Some(Context::Member {
        object: expression,
        object_span: span_of(object),
    })
}

/// K6: a token that an expression may follow, where its parent says it is
/// one: inside an `ERROR`, nothing.
fn value_after(token: Node) -> Option<Context> {
    let parent = token.parent()?;
    let value = match (token.kind(), parent.kind()) {
        ("(" | ",", "command_invocation") => true,
        ("[" | ",", "array_expression") => true,
        ("?" | ":", "conditional_expression") => true,
        (":", "object_member") => true,
        (_, "unary_expression" | "binary_expression") => {
            parent.child_by_field_name("operator").map(|op| op.id()) == Some(token.id())
        }
        _ => false,
    };
    value.then_some(Context::Value)
}

/// K2: the name right before the cursor follows `on.`, in an opening tag.
fn event_name(source: &str, offset: usize, tokens: &[Node]) -> Option<Completion> {
    let bytes = source.as_bytes();
    let is_name = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut start = offset;
    while start > 0 && is_name(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = offset;
    while end < bytes.len() && is_name(bytes[end]) {
        end += 1;
    }
    let on = start.checked_sub(3)?;
    if bytes.get(on..start) != Some(b"on.".as_slice()) {
        return None;
    }
    if on > 0 && !bytes[on - 1].is_ascii_whitespace() {
        return None;
    }
    // The opening tag the `on.` is in: scan from the last token before it.
    let last = tokens.iter().rposition(|token| token.end_byte() <= on)?;
    let tag = open_tag(last, tokens, source)?;
    Some(Completion {
        context: Context::EventName { tag },
        replace: Span {
            start_byte: start,
            end_byte: end,
        },
    })
}

/// K4: the cursor is in an opening tag, after its name or after a whole
/// attribute or binding.
fn attribute_name(p: Node, index: usize, tokens: &[Node], source: &str) -> Option<Context> {
    let ends_something = match p.kind() {
        "tag_name" => true,
        // A string value's closing quote, or a block value's closing brace.
        "\"" => p
            .parent()
            .is_some_and(|string| string.kind() == "string" && last_child(string) == Some(p.id())),
        "}" => p.parent().is_some_and(|block| {
            block.kind() == "expression_block"
                && block
                    .parent()
                    .is_some_and(|owner| matches!(owner.kind(), "attribute" | "event_binding"))
        }),
        // A lone name that didn't parse (S10).
        "identifier" => p.parent().is_some_and(|parent| parent.kind() == "ERROR"),
        _ => false,
    };
    if !ends_something {
        return None;
    }
    let tag = open_tag(index, tokens, source)?;
    Some(Context::AttributeName { tag })
}

/// The name of the opening tag that `tokens[index]` is in: scanning back,
/// the first `tag_name` right after `<`, with no `>` or `/>` on the way
/// and no `{` left open.
fn open_tag(index: usize, tokens: &[Node], source: &str) -> Option<String> {
    let mut depth = 0usize;
    for i in (0..=index).rev() {
        let token = tokens[i];
        match token.kind() {
            ">" | "/>" | "</" => return None,
            "}" => depth += 1,
            "{" => depth = depth.checked_sub(1)?,
            "tag_name" if depth == 0 => {
                let after_less_than = i
                    .checked_sub(1)
                    .is_some_and(|before| tokens[before].kind() == "<");
                return after_less_than.then(|| text_of(token, source));
            }
            _ => {}
        }
    }
    None
}

fn last_child(node: Node) -> Option<usize> {
    node.child(node.child_count().checked_sub(1)?)
        .map(|child| child.id())
}

fn floor_char_boundary(source: &str, offset: usize) -> usize {
    let mut offset = offset.min(source.len());
    while !source.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
