//! Turns the `ERROR` and `MISSING` nodes of a Tree-sitter error-recovery
//! tree into located [`ParseError`]s.
//!
//! Each error region (an outermost `ERROR` node, or a `MISSING` node) gets
//! exactly one diagnostic. A region gets a specific code only when its
//! shape unambiguously matches one of the documented mistakes below;
//! everything else falls back to `syntax-error`. Tree-sitter's recovery
//! trees are hard to predict, so every shape matched here was observed on
//! a real input and is pinned by a fixture — a correct `syntax-error`
//! always beats a wrong specific code.

use mesh_syntax::{DiagnosticCode, Span};
use tree_sitter::Node;

use crate::{span_of, ParseError};

/// Collects one [`ParseError`] per error region under `root`, in source
/// order. Only called when `root.has_error()` is true.
pub(crate) fn collect(root: Node, source: &str) -> Vec<ParseError> {
    // An empty (or whitespace-only) file parses to a single zero-width
    // `ERROR` root, which has no shape worth classifying. A leading
    // byte-order mark is not whitespace to `str::trim`, so strip it first.
    if source.trim_start_matches('\u{feff}').trim().is_empty() {
        return vec![syntax_error(
            "expected a root element, but the file is empty",
            Span {
                start_byte: 0,
                end_byte: 0,
            },
        )];
    }

    let mut errors = Vec::new();

    // Visits every outermost `ERROR` node and every `MISSING` node, in
    // source order. Never descends into an `ERROR`: nested errors belong
    // to the region that contains them.
    //
    // Iterative rather than recursive: a deeply nested tree (thousands of
    // levels, one error at the bottom) would otherwise recurse once per
    // level and blow the stack. An explicit stack holds the same walk;
    // pushing each node's children in reverse makes them pop, and so get
    // visited, in source order. `path` holds the ancestors of the node
    // being visited, root first; each stack entry remembers its depth so
    // `path` can be cut back when the walk moves to a sibling.
    let mut stack = vec![(root, 0)];
    let mut path: Vec<Node> = Vec::new();
    while let Some((node, depth)) = stack.pop() {
        path.truncate(depth);
        if node.is_error() || node.is_missing() {
            let region = Region {
                node,
                ancestors: &path,
            };
            errors.push(classify(region, source));
            continue;
        }
        if !node.has_error() {
            continue;
        }
        path.push(node);
        let mut cursor = node.walk();
        let children: Vec<Node> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev().map(|child| (child, depth + 1)));
    }

    // `has_error()` guarantees at least one region; this only guards
    // against a Tree-sitter behaviour change turning into a silent pass.
    if errors.is_empty() {
        return vec![syntax_error("invalid syntax", span_of(root))];
    }
    errors
}

/// An error region together with the path from the root down to it.
///
/// Tree-sitter's `Node::parent()` (and `prev_sibling()`/`next_sibling()`,
/// which call it) re-walks the tree from the root on every call, and for
/// a zero-width `MISSING` node it recurses once per level on the way
/// down. On a deeply nested file that is both slow and a stack overflow,
/// so the classifier never calls them: it reads parents and siblings from
/// the path the walk in [`collect`] already holds.
#[derive(Clone, Copy)]
struct Region<'path, 'tree> {
    node: Node<'tree>,
    /// Every ancestor of `node`, root first, so the parent is last.
    ancestors: &'path [Node<'tree>],
}

impl<'tree> Region<'_, 'tree> {
    /// The `n`th ancestor: 1 is the parent, 2 the grandparent, and so on.
    fn ancestor(&self, n: usize) -> Option<Node<'tree>> {
        let index = self.ancestors.len().checked_sub(n)?;
        self.ancestors.get(index).copied()
    }

    fn parent(&self) -> Option<Node<'tree>> {
        self.ancestor(1)
    }

    /// The node `offset` places after this one among its parent's
    /// children (negative for before), counting anonymous tokens too, as
    /// `Node::prev_sibling`/`next_sibling` do.
    fn sibling(&self, offset: isize) -> Option<Node<'tree>> {
        let parent = self.parent()?;
        let mut cursor = parent.walk();
        let siblings: Vec<Node> = parent.children(&mut cursor).collect();
        let index = siblings.iter().position(|n| n.id() == self.node.id())?;
        siblings.get(index.checked_add_signed(offset)?).copied()
    }
}

fn classify(region: Region, source: &str) -> ParseError {
    if region.node.is_missing() {
        classify_missing(region, source)
    } else {
        classify_error(region, source)
    }
}

fn classify_missing(region: Region, source: &str) -> ParseError {
    let missing = region.node;
    let Some(parent) = region.parent() else {
        return syntax_error("invalid syntax", span_of(missing));
    };

    // `<name ...` never finished: Tree-sitter closes the element with a
    // zero-width `/>`. The same shape comes from a literal `<` in text
    // (`<p>a < b</p>` reads `< b` as an unfinished nested element), which
    // is told apart by the whitespace right after the `<`.
    if missing.kind() == "/>" && parent.kind() == "self_closing_element" {
        return unfinished_open_tag(region, parent, source);
    }

    // An operand is missing inside an expression: `{a +}`, or the slot
    // after a trailing comma in `save(a, b,)`.
    if parent.kind() == "expression" {
        if let Some(comma) = trailing_command_comma(region, source) {
            return command_trailing_comma(comma);
        }
        return syntax_error("expected an expression", span_of(missing));
    }

    syntax_error("invalid syntax", span_of(missing))
}

fn classify_error(region: Region, source: &str) -> ParseError {
    let error = region.node;
    let parent_kind = region.parent().map(|parent| parent.kind());

    if let Some(diagnostic) = malformed_event_binding(region, source) {
        return diagnostic;
    }

    if parent_kind == Some("container_element") {
        if let Some(less_than) = less_than_in_text(region, source) {
            return less_than_diagnostic(less_than);
        }
    }

    if matches!(
        parent_kind,
        Some("attribute" | "self_closing_element" | "container_element")
    ) {
        if let Some(diagnostic) = hyphenated_attribute_name(error, source) {
            return diagnostic;
        }
    }

    if parent_kind == Some("expression_block") && is_single_brace_object(region) {
        let block = region.parent().expect("parent_kind is Some");
        return ParseError {
            code: DiagnosticCode::SINGLE_BRACE_OBJECT,
            message: "an object needs its own braces inside `{...}`: write `{{ key: value }}`"
                .to_string(),
            span: span_of(block),
        };
    }

    if parent_kind == Some("command_invocation")
        && error.utf8_text(source.as_bytes()) == Ok(",")
        && region.sibling(-1).map(|n| n.kind()) == Some("expression")
        && region.sibling(1).map(|n| n.kind()) == Some(")")
    {
        return command_trailing_comma(span_of(error));
    }

    if let Some(diagnostic) = open_tag_in_error(error, source) {
        return diagnostic;
    }

    if parent_kind == Some("source_file") && starts_with_kind(error, "<") {
        let next_root = region.sibling(1).filter(|n| n.kind() == "element");
        let prev_root = region.sibling(-1).filter(|n| n.kind() == "element");
        if next_root.is_some() || prev_root.is_some() {
            // Point at whichever root comes second: that's the extra one.
            let extra = next_root.unwrap_or(error);
            return syntax_error(
                "expected a single root element, found another one here",
                span_of(extra),
            );
        }
    }

    syntax_error("invalid syntax", span_of(error))
}

/// `on.={x}`, `on.a.b={x}`, `on.my-event={x}`, `on.click="x"`: an `ERROR`
/// inside an `event_binding`, or one that starts with the `on.` prefix.
fn malformed_event_binding(region: Region, source: &str) -> Option<ParseError> {
    let error = region.node;
    let parent = region.parent()?;
    let start = if parent.kind() == "event_binding" {
        parent.start_byte()
    } else if source[error.start_byte()..].starts_with("on.") {
        error.start_byte()
    } else {
        return None;
    };

    // Underline `on.` plus whatever the author wrote as the event name.
    let name_start = start + "on.".len();
    let end = name_start
        + source[name_start..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')))
            .unwrap_or(source.len() - name_start);

    Some(ParseError {
        code: DiagnosticCode::MALFORMED_EVENT_BINDING,
        message: format!(
            "malformed event binding `{}`: expected `on.<event>={{handler}}`",
            &source[start..end]
        ),
        span: Span {
            start_byte: start,
            end_byte: end,
        },
    })
}

/// Finds a `<` token inside a text-position `ERROR` that reads as a
/// comparison: it comes after the element's opening `>` and is followed
/// by whitespace, `=`, or a digit (`a < b`, `a <= b`, `x<5`). Anything
/// else after the `<` (`<!--`, `<$`) is some other mistake, so it's left
/// to the `syntax-error` fallback.
fn less_than_in_text(region: Region, source: &str) -> Option<Span> {
    let error = region.node;
    let element = region.parent()?;
    let mut cursor = error.walk();
    let less_than = error.children(&mut cursor).find(|token| {
        token.kind() == "<"
            && source[element.start_byte()..token.start_byte()].contains('>')
            && source[token.end_byte()..]
                .starts_with(|c: char| c.is_whitespace() || c == '=' || c.is_ascii_digit())
    })?;
    Some(span_of(less_than))
}

fn less_than_diagnostic(span: Span) -> ParseError {
    ParseError {
        code: DiagnosticCode::LESS_THAN_IN_TEXT,
        message: "`<` in text starts a tag; to show a literal `<`, put the text in a string \
                  expression, like `{\"a < b\"}`"
            .to_string(),
        span,
    }
}

/// `data-id="x"`: the `-` splits the attribute name, leaving an `ERROR`
/// holding a `-` token in the element's opening tag. The hyphenated word
/// must be followed by `=`, like an attribute; any other `-` (such as the
/// `<!--` of an HTML comment) is left to the `syntax-error` fallback.
fn hyphenated_attribute_name(error: Node, source: &str) -> Option<ParseError> {
    let mut cursor = error.walk();
    let hyphen = error
        .children(&mut cursor)
        .find(|token| token.kind() == "-")?;

    // Widen from the `-` to the whole hyphenated word, e.g. `data-id`.
    let is_name_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '_' | '-');
    let start = source[..hyphen.start_byte()]
        .char_indices()
        .rev()
        .find(|&(_, c)| !is_name_char(c))
        .map_or(0, |(i, c)| i + c.len_utf8());
    // An attribute name starts after whitespace or right after the
    // previous attribute's value (`b="x"c-d=`, `b={x}c-d=`). Anything else
    // just before the word is a second mistake the specific code wouldn't
    // cover: `é` in `é-id` can't be in a name either, and a non-ASCII
    // space like U+00A0 isn't whitespace to the grammar, whose `extras`
    // (`/\s/`) only skip ASCII whitespace, `\x0b` and `\x0c` included.
    let starts_a_name =
        |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c' | '"' | '}');
    if !source[..start].ends_with(starts_a_name) {
        return None;
    }
    let end = hyphen.start_byte()
        + source[hyphen.start_byte()..]
            .find(|c: char| !is_name_char(c))
            .unwrap_or(source.len() - hyphen.start_byte());
    if !source[end..].trim_start().starts_with('=') {
        return None;
    }

    Some(ParseError {
        code: DiagnosticCode::HYPHENATED_ATTRIBUTE_NAME,
        message: format!(
            "attribute name `{}` can't contain `-`; use camelCase or `_` instead",
            &source[start..end]
        ),
        span: Span {
            start_byte: start,
            end_byte: end,
        },
    })
}

/// `data={ k: "v" }`: the block's first thing is a key followed by `:`.
/// Tree-sitter leaves either `k:` in the `ERROR` (identifier key) or just
/// `: "v"` after a string-key expression (`{ "k": "v" }`).
fn is_single_brace_object(region: Region) -> bool {
    let error = region.node;
    let first = error.child(0);
    let first_kind = first.map(|n| n.kind());
    let second_kind = error.child(1).map(|n| n.kind());

    // The key must be the first thing in the block, right after its `{`.
    let (key, before_key) = match (first_kind, second_kind) {
        (Some("expression"), Some(":")) => (first, region.sibling(-1)),
        (Some(":"), _) => {
            let key = region.sibling(-1).filter(|n| n.kind() == "expression");
            (key, key.and(region.sibling(-2)))
        }
        _ => return false,
    };

    before_key.map(|n| n.kind()) == Some("{") && key.is_some_and(is_object_key)
}

/// An expression that could have been meant as an object key: a bare
/// name or a string literal.
fn is_object_key(expression: Node) -> bool {
    match expression.named_child(0) {
        Some(inner) if inner.kind() == "reference" => true,
        Some(inner) if inner.kind() == "literal" => {
            inner.named_child(0).map(|n| n.kind()) == Some("string")
        }
        _ => false,
    }
}

/// The comma before a missing operand, when that operand would have been
/// the last argument of a command: `save(a, b,)`.
fn trailing_command_comma(region: Region, source: &str) -> Option<Span> {
    let missing = region.node;
    let ancestor = region
        .ancestors
        .iter()
        .rev()
        .find(|ancestor| ancestor.kind() != "expression")?;
    if ancestor.kind() != "command_invocation" {
        return None;
    }

    let before = source[..missing.start_byte()].trim_end();
    let after = source[missing.end_byte()..].trim_start();
    if !(before.ends_with(',') && after.starts_with(')')) {
        return None;
    }
    Some(Span {
        start_byte: before.len() - 1,
        end_byte: before.len(),
    })
}

fn command_trailing_comma(span: Span) -> ParseError {
    ParseError {
        code: DiagnosticCode::COMMAND_TRAILING_COMMA,
        message: "trailing comma in command arguments; remove the `,`".to_string(),
        span,
    }
}

/// A self-closing element Tree-sitter had to finish with a `MISSING /> `.
fn unfinished_open_tag(region: Region, element: Node, source: &str) -> ParseError {
    let less_than = element
        .child(0)
        .expect("a self_closing_element always starts with `<`");

    // `a < b` in text: the element is nested (so it sits in text) and the
    // `<` is followed by whitespace, which a real tag never is.
    // `element` is the `MISSING` node's parent, so its grandparent is
    // the `MISSING` node's third ancestor.
    let nested = region.ancestor(3).map(|n| n.kind()) == Some("child");
    if nested && source[less_than.end_byte()..].starts_with(char::is_whitespace) {
        return less_than_diagnostic(span_of(less_than));
    }

    let name = element
        .child_by_field_name("name")
        .expect("a self_closing_element always has a name");
    unterminated_tag(less_than, name, source)
}

/// An `ERROR` that starts like an element (`<` then a tag name) and is
/// missing either its `>`/`/>` or its closing tag.
fn open_tag_in_error(error: Node, source: &str) -> Option<ParseError> {
    let less_than = error.child(0).filter(|n| n.kind() == "<")?;
    let name = error.child(1).filter(|n| n.kind() == "tag_name")?;

    let mut cursor = error.walk();
    let kinds: Vec<&str> = error.children(&mut cursor).map(|n| n.kind()).collect();

    if !kinds.contains(&">") && !kinds.contains(&"/>") {
        return Some(unterminated_tag(less_than, name, source));
    }

    // `<page> ...` with no `</page>` anywhere after it. If the closing
    // tag's text does appear, Tree-sitter paired it with something else
    // and guessing which element is really unclosed would be a guess. A
    // `>` straight after a `/` is the end of a broken `/>`, not an
    // opening tag, so that shape is a guess too.
    let name_text = &source[name.start_byte()..name.end_byte()];
    let region = &source[error.start_byte()..error.end_byte()];
    let mut cursor = error.walk();
    let open_end = error.children(&mut cursor).find(|n| n.kind() == ">")?;
    if kinds.contains(&"</")
        || region.contains(&format!("</{name_text}"))
        || source[..open_end.start_byte()].ends_with('/')
    {
        return None;
    }

    Some(ParseError {
        code: DiagnosticCode::MISSING_CLOSING_TAG,
        message: format!("`<{name_text}>` is never closed: expected `</{name_text}>`"),
        span: Span {
            start_byte: less_than.start_byte(),
            end_byte: open_end.end_byte(),
        },
    })
}

fn unterminated_tag(less_than: Node, name: Node, source: &str) -> ParseError {
    let name_text = &source[name.start_byte()..name.end_byte()];
    ParseError {
        code: DiagnosticCode::UNTERMINATED_TAG,
        message: format!("unterminated tag `<{name_text}`: expected `>` or `/>`"),
        span: Span {
            start_byte: less_than.start_byte(),
            end_byte: name.end_byte(),
        },
    }
}

fn starts_with_kind(node: Node, kind: &str) -> bool {
    node.child(0).map(|n| n.kind()) == Some(kind)
}

fn syntax_error(message: &str, span: Span) -> ParseError {
    ParseError {
        code: DiagnosticCode::SYNTAX_ERROR,
        message: message.to_string(),
        span,
    }
}
