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

use crate::{lower_expression, lower_member_access, lower_reference, nesting, span_of, text_of};

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
    /// The name of a snippet, after `{@render `.
    Render,
    /// Inside the quotes of a string that is a closed set's value, or may be: the text between them is what a completion replaces.
    Text(TextSite),
}

/// Where a string literal is, for the values a closed set offers there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextSite {
    /// The value of the prop `prop` of an element named `tag`, written `prop="…"` or `prop={"…"}`.
    Prop { tag: String, prop: String },
    /// An operand of `==` or `!=`; `other` is the operand beside it.
    Compared { other: Expression, other_span: Span },
}

/// A snippet declared among the children of an element or a block, which a render beside it can name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSnippet {
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<String>,
    /// Where each parameter is declared, in order.
    pub parameter_spans: Vec<Span>,
}

/// The `{@render name(` the cursor is inside the arguments of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderCall {
    pub name: String,
    pub name_span: Span,
    /// How many arguments are complete before the cursor: the index of the one being written.
    pub active: usize,
}

/// What a name written at the cursor can mean beyond the template's scope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Reach {
    /// Local snippets in reach, outermost first, in the order declared.
    pub snippets: Vec<LocalSnippet>,
    /// Names bound around the cursor by a snippet's parameters and an `{#each}`'s item, innermost first.
    pub bindings: Vec<String>,
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
    // Inside the quotes of a string that is a prop's value or an operand of a comparison.
    if let Some(completion) = text_value(root, source, offset, &tokens) {
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
        // K7: a block's condition or list starts right after its opener.
        "if_open" | "each_open" | "else_if_open" => Some(Context::Value),
        // The name of the snippet a render tag renders.
        "render_open" => Some(Context::Render),
        "(" | "," | "[" | "?" | ":" | "!" | "-" | "+" | "*" | "/" | "%" | "<=" | ">=" | "=="
        | "!=" | "&&" | "||" | "??" | ">" => value_after(p),
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
        // K7: the key of an `{#each}`.
        ("(", "each_block") => true,
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

/// The string, with the cursor inside its quotes (or in its content), that is a prop's value or an operand of `==` or `!=`.
fn text_value(root: Node, source: &str, offset: usize, tokens: &[Node]) -> Option<Completion> {
    let last = *tokens.last()?;
    let string = match last.kind() {
        "string_content" if last.start_byte() < offset && offset <= last.end_byte() => {
            last.parent()?
        }
        // An empty string: the cursor between the quotes.
        "\"" if last.end_byte() == offset => {
            let string = last.parent()?;
            let first = string.child(0)?;
            let closed = string.child_count() >= 2 && string.end_byte() > offset;

            (string.kind() == "string" && first.id() == last.id() && closed).then_some(string)?
        }
        _ => return None,
    };
    if string.kind() != "string" || string.has_error() {
        return None;
    }
    let replace = string
        .child_by_field_name("value")
        .map(span_of)
        .unwrap_or(Span {
            start_byte: offset,
            end_byte: offset,
        });
    let site = string_site(root, string, source)?;

    Some(Completion {
        context: Context::Text(site),
        replace,
    })
}

fn string_site(_root: Node, string: Node, source: &str) -> Option<TextSite> {
    let parent = string.parent()?;
    match parent.kind() {
        "attribute" if parent.child_by_field_name("value")?.id() == string.id() => {
            attribute_site(parent, source)
        }
        "literal" => {
            let expression = parent.parent().filter(|node| node.kind() == "expression")?;
            let owner = expression.parent()?;
            match owner.kind() {
                // `prop={"…"}`.
                "expression_block" => {
                    let attribute = owner.parent().filter(|node| node.kind() == "attribute")?;
                    attribute_site(attribute, source)
                }
                "binary_expression" => {
                    let operator = owner.child_by_field_name("operator")?;
                    if !matches!(
                        &source[operator.start_byte()..operator.end_byte()],
                        "==" | "!="
                    ) {
                        return None;
                    }
                    let left = owner.child_by_field_name("left")?;
                    let right = owner.child_by_field_name("right")?;
                    let other = if left.id() == expression.id() {
                        right
                    } else {
                        left
                    };

                    Some(TextSite::Compared {
                        other: lower_expression(other, source),
                        other_span: span_of(other),
                    })
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn attribute_site(attribute: Node, source: &str) -> Option<TextSite> {
    let prop = text_of(attribute.child_by_field_name("name")?, source);
    let element = attribute.parent()?;
    let tag = text_of(element.child_by_field_name("name")?, source);

    Some(TextSite::Prop { tag, prop })
}

/// The snippets in reach of byte `offset` and the names bound around it (see [`Reach`]): read from the tree, so the file has to parse apart from the name being typed.
pub fn reach_at(source: &str, offset: usize) -> Reach {
    let offset = floor_char_boundary(source, offset);
    let mut parser = tree_sitter::Parser::new();
    let mut reach = Reach::default();
    if parser.set_language(&tree_sitter_mprx::language()).is_err() {
        return reach;
    }
    let Some(tree) = parser.parse(source, None) else {
        return reach;
    };
    let root = tree.root_node();
    if nesting::check(root).is_some() {
        return reach;
    }
    let Some(mut node) = root.descendant_for_byte_range(offset, offset) else {
        return reach;
    };
    let mut outer_first: Vec<Vec<LocalSnippet>> = Vec::new();
    loop {
        // Names this block binds, for what is after its header.
        match node.kind() {
            "snippet_block" if after_header(node, offset) => {
                let mut cursor = node.walk();
                for name in node.children_by_field_name("parameter", &mut cursor) {
                    reach.bindings.push(text_of(name, source));
                }
            }
            "each_block" => {
                if let Some(binding) = node.child_by_field_name("binding") {
                    if binding.end_byte() <= offset {
                        reach.bindings.push(text_of(binding, source));
                    }
                }
            }
            _ => {}
        }
        // The snippets declared among this node's children, when the cursor is in them.
        if after_header(node, offset) {
            let mut declared = Vec::new();
            let mut cursor = node.walk();
            for child in node
                .children(&mut cursor)
                .filter(|child| child.kind() == "child")
            {
                let Some(block) = child
                    .child(0)
                    .filter(|block| block.kind() == "snippet_block")
                else {
                    continue;
                };
                let Some(name) = block.child_by_field_name("name") else {
                    continue;
                };
                let mut parameters = Vec::new();
                let mut parameter_spans = Vec::new();
                let mut inner = block.walk();
                for parameter in block.children_by_field_name("parameter", &mut inner) {
                    parameters.push(text_of(parameter, source));
                    parameter_spans.push(span_of(parameter));
                }
                declared.push(LocalSnippet {
                    name: text_of(name, source),
                    name_span: span_of(name),
                    parameters,
                    parameter_spans,
                });
            }
            if !declared.is_empty() {
                outer_first.push(declared);
            }
        }
        match node.parent() {
            Some(parent) => node = parent,
            None => break,
        }
    }
    reach.snippets = outer_first.into_iter().rev().flatten().collect();
    reach
}

/// Whether `offset` is in the part of `node` after its opening (the children's part), for a node that has children.
fn after_header(node: Node, offset: usize) -> bool {
    let opener = match node.kind() {
        "container_element" => ">",
        "snippet_block" | "each_block" | "if_block" | "else_if_clause" | "else_clause" => "}",
        _ => return node.kind() == "source_file",
    };
    let mut cursor = node.walk();
    let header = node
        .children(&mut cursor)
        .find(|child| child.kind() == opener);

    header.is_some_and(|header| header.end_byte() <= offset)
}

/// The render tag whose arguments contain byte `offset`: the unmatched `(` before it, right after
/// `{@render name`. Read from the tokens before the cursor, so it answers while the call is being
/// typed, closed or not.
pub fn render_call_at(source: &str, offset: usize) -> Option<RenderCall> {
    let offset = floor_char_boundary(source, offset);
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_mprx::language()).ok()?;
    let tree = parser.parse(source, None)?;
    let root = tree.root_node();
    if nesting::check(root).is_some() {
        return None;
    }
    let tokens = tokens_before(root, offset);
    let mut depth = 0usize;
    let mut commas = 0usize;
    for index in (0..tokens.len()).rev() {
        let token = tokens[index];
        match token.kind() {
            ")" | "]" | "}" => depth += 1,
            "[" | "{" => {
                // An opener of something other than the call: the cursor is not directly in its arguments.
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            "(" => {
                if depth > 0 {
                    depth -= 1;
                    continue;
                }
                let name = *tokens.get(index.checked_sub(1)?)?;
                let opener = *tokens.get(index.checked_sub(2)?)?;
                if name.kind() != "identifier" || opener.kind() != "render_open" {
                    return None;
                }
                return Some(RenderCall {
                    name: text_of(name, source),
                    name_span: span_of(name),
                    active: commas,
                });
            }
            "," if depth == 0 => commas += 1,
            _ => {}
        }
    }
    None
}
