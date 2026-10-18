//! The mistakes in blocks (`{#if}`, `{:else if}`, `{:else}`, `{/if}`, `{#each}`, `{/each}`, `{#snippet}`, `{/snippet}`, `{@render}`), found by a
//! scan of the source text.
//!
//! Tree-sitter's recovery tree says only that something near a block is wrong, and where it puts the
//! error depends on what follows. A block's delimiters are single tokens that cannot start anything
//! else, so the delimiters themselves say what is wrong: a block never closed, a closer or `{:else}`
//! with no open block of its kind, an opener that is not well formed, a block where the document
//! must be an element. The scan reads the source as MPRX is read (text, tags, strings and braced
//! expressions) and reports each such mistake at the block, with a message that says what to write.
//!
//! It runs only on a source whose tree has errors, so it never has to accept or reject a valid file.

use mesh_syntax::{DiagnosticCode, Span};

use crate::ParseError;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    If,
    Each,
    Snippet,
}

impl Kind {
    fn opener(self) -> &'static str {
        match self {
            Kind::If => "{#if}",
            Kind::Each => "{#each}",
            Kind::Snippet => "{#snippet}",
        }
    }

    fn closer(self) -> &'static str {
        match self {
            Kind::If => "{/if}",
            Kind::Each => "{/each}",
            Kind::Snippet => "{/snippet}",
        }
    }
}

struct Open {
    kind: Kind,
    /// Where the opener is, for "never closed".
    span: Span,
    /// The elements open around it: it must close before any of them does.
    depth: usize,
    /// Whether `{:else}` has been written.
    otherwise: bool,
}

fn error(message: impl Into<String>, start: usize, end: usize) -> ParseError {
    ParseError {
        code: DiagnosticCode::SYNTAX_ERROR,
        message: message.into(),
        span: Span {
            start_byte: start,
            end_byte: end,
        },
    }
}

/// The index just past the `}` that closes the `{` at `from`, strings respected; `None` when it never closes.
fn braced(bytes: &[u8], from: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut at = from;

    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
        at += 1;
    }

    None
}

/// The index just past a tag that starts at `<` at `from`, and whether it closes itself.
fn tag_end(bytes: &[u8], from: usize) -> (usize, bool) {
    let mut at = from + 1;

    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' => {
                // An expression value; skip it whole.
                at = braced(bytes, at).unwrap_or(bytes.len());
                continue;
            }
            b'>' => return (at + 1, bytes[at - 1] == b'/'),
            _ => {}
        }
        at += 1;
    }

    (bytes.len(), false)
}

/// Whether `text` is a name: `[A-Za-z_][A-Za-z0-9_]*`.
fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The position of the last ` as ` that is not inside brackets, braces or a string, in `head`.
fn last_top_level_as(head: &str) -> Option<usize> {
    let bytes = head.as_bytes();
    let mut depth = 0i32;
    let mut found = None;
    let mut at = 0;

    while at < bytes.len() {
        match bytes[at] {
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            _ if depth == 0
                && bytes[at].is_ascii_whitespace()
                && head[at..].trim_start().starts_with("as")
                && head[at..]
                    .trim_start()
                    .get(2..3)
                    .is_some_and(|next| next.starts_with(|c: char| c.is_ascii_whitespace())) =>
            {
                found = Some(at);
            }
            _ => {}
        }
        at += 1;
    }

    found
}

/// Checks the text after `{#each`: `items as item` with an optional `(key)`.
fn check_each(inner: &str, start: usize, end: usize) -> Option<ParseError> {
    let malformed = |why: &str| {
        Some(error(
            format!("malformed `{{#each}}`: {why}; write `{{#each items as item (key)}}`"),
            start,
            end,
        ))
    };
    if inner.trim().is_empty() {
        return malformed("it has no list");
    }
    let Some(at) = last_top_level_as(inner) else {
        return malformed("`as` and the name for one item are missing");
    };
    let items = &inner[..at];
    let rest = inner[at..].trim_start()[2..].trim();

    if items.trim().is_empty() {
        return malformed("it has no list before `as`");
    }

    let (name, key) = match rest.find('(') {
        Some(open) => {
            let key = rest[open..].trim();
            if !key.ends_with(')') {
                return malformed("the key's `(` is never closed");
            }
            (rest[..open].trim(), Some(&key[1..key.len() - 1]))
        }
        None => (rest, None),
    };

    if !is_name(name) {
        return malformed("`as` must be followed by a name for one item");
    }
    if key.is_some_and(|key| key.trim().is_empty()) {
        return malformed("the key is empty");
    }

    None
}

/// Checks the text after `{#snippet`: `name(parameter, …)`.
fn check_snippet(inner: &str, start: usize, end: usize) -> Option<ParseError> {
    let malformed = |why: &str| {
        Some(error(
            format!("malformed `{{#snippet}}`: {why}; write `{{#snippet name(parameters)}}`"),
            start,
            end,
        ))
    };
    let inner = inner.trim();
    let Some(open) = inner.find('(') else {
        return malformed("it has no parameter list");
    };
    if !is_name(inner[..open].trim()) {
        return malformed("it has no name");
    }
    let Some(list) = inner[open + 1..].strip_suffix(')') else {
        return malformed("the parameter list's `(` is never closed");
    };
    if !list.trim().is_empty() && !list.split(',').all(|parameter| is_name(parameter.trim())) {
        return malformed("each parameter is a name");
    }

    None
}

/// Checks the text after `{@render`: `name(argument, …)`.
fn check_render(inner: &str, start: usize, end: usize) -> Option<ParseError> {
    let malformed = |why: &str| {
        Some(error(
            format!("malformed `{{@render}}`: {why}; write `{{@render name(arguments)}}`"),
            start,
            end,
        ))
    };
    let inner = inner.trim();
    let Some(open) = inner.find('(') else {
        return malformed("it has no argument list");
    };
    if !is_name(inner[..open].trim()) {
        return malformed("it has no snippet name");
    }
    if !inner.ends_with(')') {
        return malformed("the argument list's `(` is never closed");
    }

    None
}

/// Every block mistake in `source`, in source order. Empty when the scan finds none.
pub(crate) fn scan(source: &str) -> Vec<ParseError> {
    let bytes = source.as_bytes();
    let mut errors = Vec::new();
    let mut open: Vec<Open> = Vec::new();
    let mut depth = 0usize;
    let mut at = 0;

    while at < bytes.len() {
        match bytes[at] {
            b'<' if bytes.get(at + 1) == Some(&b'/') => {
                // A block opened inside this element must have closed before it.
                while open
                    .last()
                    .is_some_and(|block| block.depth >= depth && depth > 0)
                {
                    let block = open.pop().expect("checked by the loop condition");
                    errors.push(error(
                        format!(
                            "`{}` is never closed: expected `{}` before the end of the element it is in",
                            block.kind.opener(),
                            block.kind.closer()
                        ),
                        block.span.start_byte,
                        block.span.end_byte,
                    ));
                }
                depth = depth.saturating_sub(1);
                at = source[at..]
                    .find('>')
                    .map_or(bytes.len(), |end| at + end + 1);
            }
            b'<' if bytes
                .get(at + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_') =>
            {
                let (end, closes) = tag_end(bytes, at);
                if !closes {
                    depth += 1;
                }
                at = end;
            }
            b'{' => match bytes.get(at + 1) {
                Some(b'#') => {
                    let end = braced(bytes, at);
                    let word_end = source[at + 2..]
                        .find(|c: char| !c.is_ascii_alphabetic())
                        .map_or(bytes.len(), |n| at + 2 + n);
                    let word = &source[at + 2..word_end];
                    let stop = end.unwrap_or(bytes.len());
                    let kind = match word {
                        "if" => Some(Kind::If),
                        "each" => Some(Kind::Each),
                        "snippet" => Some(Kind::Snippet),
                        _ => None,
                    };
                    let Some(kind) = kind else {
                        errors.push(error(
                            format!("unknown block `{{#{word}`: the blocks are `{{#if}}`, `{{#each}}` and `{{#snippet}}`"),
                            at,
                            word_end,
                        ));
                        at = stop;
                        continue;
                    };
                    let Some(end) = end else {
                        errors.push(error(
                            format!("`{{#{word}` is never finished: expected `}}`"),
                            at,
                            word_end,
                        ));
                        break;
                    };
                    let inner = &source[word_end..end - 1];
                    let opener = Span {
                        start_byte: at,
                        end_byte: end,
                    };

                    if depth == 0 && open.is_empty() {
                        errors.push(error(
                            "a block cannot be the document: put it inside an element",
                            at,
                            word_end,
                        ));
                    }
                    match kind {
                        Kind::If => {
                            if inner.trim().is_empty() {
                                errors.push(error(
                                    "malformed `{#if}`: it has no condition; write `{#if condition}`",
                                    at,
                                    end,
                                ));
                            }
                        }
                        Kind::Each => errors.extend(check_each(inner, at, end)),
                        Kind::Snippet => errors.extend(check_snippet(inner, at, end)),
                    }
                    open.push(Open {
                        kind,
                        span: opener,
                        depth,
                        otherwise: false,
                    });
                    at = end;
                }
                Some(b':') => {
                    let end = braced(bytes, at).unwrap_or(bytes.len());
                    let inner = source[at + 2..end.saturating_sub(1).max(at + 2)].trim();
                    let span = Span {
                        start_byte: at,
                        end_byte: end,
                    };
                    let (is_else_if, condition) = match inner.strip_prefix("else") {
                        Some("") => (false, ""),
                        Some(rest) if rest.starts_with(|c: char| c.is_ascii_whitespace()) => {
                            match rest.trim_start().strip_prefix("if") {
                                Some(condition)
                                    if condition.is_empty()
                                        || condition
                                            .starts_with(|c: char| c.is_ascii_whitespace()) =>
                                {
                                    (true, condition.trim())
                                }
                                _ => {
                                    errors.push(error(
                                        "malformed `{:else}`: write `{:else}` or `{:else if condition}`",
                                        at,
                                        end,
                                    ));
                                    at = end;
                                    continue;
                                }
                            }
                        }
                        _ => {
                            errors.push(error(
                                format!(
                                    "unknown `{{:{}`: inside `{{#if}}` write `{{:else if condition}}` or `{{:else}}`",
                                    inner.split_whitespace().next().unwrap_or("")
                                ),
                                at,
                                end,
                            ));
                            at = end;
                            continue;
                        }
                    };
                    let word = if is_else_if { "{:else if}" } else { "{:else}" };

                    match open.last_mut() {
                        Some(block) if block.kind == Kind::If => {
                            if block.otherwise {
                                errors.push(error(
                                    format!("`{word}` after `{{:else}}`: `{{:else}}` is the last branch of `{{#if}}`"),
                                    span.start_byte,
                                    span.end_byte,
                                ));
                            }
                            if is_else_if && condition.is_empty() {
                                errors.push(error(
                                    "malformed `{:else if}`: it has no condition; write `{:else if condition}`",
                                    span.start_byte,
                                    span.end_byte,
                                ));
                            }
                            if !is_else_if {
                                block.otherwise = true;
                            }
                        }
                        _ => errors.push(error(
                            format!("`{word}` with no open `{{#if}}`"),
                            span.start_byte,
                            span.end_byte,
                        )),
                    }
                    at = end;
                }
                Some(b'/') => {
                    let end = braced(bytes, at).unwrap_or(bytes.len());
                    let word = source[at + 2..end.saturating_sub(1).max(at + 2)].trim();
                    let span = Span {
                        start_byte: at,
                        end_byte: end,
                    };
                    let kind = match word {
                        "if" => Some(Kind::If),
                        "each" => Some(Kind::Each),
                        "snippet" => Some(Kind::Snippet),
                        _ => None,
                    };

                    match (kind, open.last()) {
                        (None, _) => errors.push(error(
                            format!("unknown closer `{{/{word}}}`: the closers are `{{/if}}`, `{{/each}}` and `{{/snippet}}`"),
                            span.start_byte,
                            span.end_byte,
                        )),
                        (Some(kind), None) => errors.push(error(
                            format!("`{}` with no open `{}`", kind.closer(), kind.opener()),
                            span.start_byte,
                            span.end_byte,
                        )),
                        (Some(kind), Some(block)) if block.kind != kind => {
                            let expected = block.kind;
                            let opened = block.span;
                            errors.push(error(
                                format!(
                                    "`{}` closes a block that is not open: the open block is `{}` (at byte {}), which ends with `{}`",
                                    kind.closer(),
                                    expected.opener(),
                                    opened.start_byte,
                                    expected.closer()
                                ),
                                span.start_byte,
                                span.end_byte,
                            ));
                            // The closer was meant for something; leave the open block to its own closer.
                        }
                        (Some(_), Some(_)) => {
                            open.pop();
                        }
                    }
                    at = end;
                }
                Some(b'@') => {
                    let end = braced(bytes, at);
                    let word_end = source[at + 2..]
                        .find(|c: char| !c.is_ascii_alphabetic())
                        .map_or(bytes.len(), |n| at + 2 + n);
                    let word = &source[at + 2..word_end];
                    let stop = end.unwrap_or(bytes.len());

                    if word != "render" {
                        errors.push(error(
                            format!("unknown tag `{{@{word}`: the only tag is `{{@render name(arguments)}}`"),
                            at,
                            word_end,
                        ));
                    } else if let Some(end) = end {
                        if depth == 0 && open.is_empty() {
                            errors.push(error(
                                "a render tag cannot be the document: put it inside an element",
                                at,
                                word_end,
                            ));
                        }
                        errors.extend(check_render(&source[word_end..end - 1], at, end));
                    } else {
                        errors.push(error(
                            "`{@render` is never finished: expected `}`",
                            at,
                            word_end,
                        ));
                    }
                    at = stop;
                }
                _ => at = braced(bytes, at).unwrap_or(bytes.len()),
            },
            _ => at += 1,
        }
    }

    for block in open.into_iter().rev() {
        errors.push(error(
            format!(
                "`{}` is never closed: expected `{}`",
                block.kind.opener(),
                block.kind.closer()
            ),
            block.span.start_byte,
            block.span.end_byte,
        ));
    }

    errors.sort_by_key(|error| error.span.start_byte);
    errors
}

/// The mistakes a well-formed block tree can still hold. The grammar reads the word between a repeat's list and its name as
/// any identifier (a keyword token there would change how every other syntax error is recovered), so it is checked to be `as`.
pub(crate) fn check_tree(root: tree_sitter::Node, source: &str) -> Vec<ParseError> {
    let mut errors = Vec::new();
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        if node.kind() == "each_block" {
            let mut cursor = node.walk();
            let word = node
                .children(&mut cursor)
                .find(|child| child.kind() == "as_keyword");

            if let Some(word) = word {
                let text = &source[word.start_byte()..word.end_byte()];

                if text != "as" {
                    errors.push(error(
                        format!(
                            "malformed `{{#each}}`: expected `as` before the item's name, found `{text}`; write `{{#each items as item (key)}}`"
                        ),
                        word.start_byte(),
                        word.end_byte(),
                    ));
                }
            }
        }
        let mut cursor = node.walk();
        let children: Vec<_> = node.children(&mut cursor).collect();
        stack.extend(children.into_iter().rev());
    }

    errors.sort_by_key(|error| error.span.start_byte);
    errors
}
