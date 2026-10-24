//! Checking a source that is still arriving (decisions G1 to G14 of the Valance direction work; Bet 13).
//!
//! A batch check of half a file reports a syntax error and nothing else (275 prefixes of 276 in the measurement), so a wrong prop written early is invisible
//! until the file ends. This module checks the **repaired prefix**: the source cut back to its last complete token, with what is open closed, so the
//! checker sees a whole file and reports what is wrong with the part that has arrived. The repair is the checker's, not the host's, and the model never sees
//! the closers it adds.
//!
//! - [`repair`] is the repair, a scanner over the delimiters of the language and nothing else. It keeps a trailing run of text that is still growing, but only
//!   inside an element (or block) whose opening is complete; it never keeps anything cut inside a start tag, an attribute string, an expression or a block
//!   header (closing a cut attribute string checks clean with a different value in 39 of 40 prefixes, so drawing it would show wrong data); and it withholds an
//!   open `{#each}`, because a repeat with a body of more than one element is not a prefix of its final tree (decision G11).
//! - [`step`] checks the repaired prefix against a [`Model`]: the diagnostics standing, the template when there is none, what is open.
//! - [`Session`] is `open`, `push` and `close`: each `push` reports the diagnostics that are new (a mistake is reported once, at the first prefix that
//!   contains it), and `close` is **exactly the batch check of the whole text**, whatever was pushed and however it was cut (decision G4: property-tested).
//!
//! The preview is a *fresh* render of the template of each step: a repaired prefix is a different program from the next one, and node identity includes the
//! program, so no node keeps its identity between steps (decision G11). Nothing here renders; it hands the host a template that has been checked.

use crate::check::{self, Model};
use mesh_syntax::{Diagnostic, Span};
use std::collections::HashSet;

/// What is open at the end of a prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PendingKind {
    /// An element whose start tag is complete and whose end tag has not arrived.
    Element,
    /// An `{#if}` whose `{/if}` has not arrived.
    If,
    /// An `{#each}` whose `{/each}` has not arrived. It is withheld from the preview.
    Each,
    /// A `{#snippet}` whose `{/snippet}` has not arrived.
    Snippet,
}

impl PendingKind {
    /// The name the JSON document uses.
    pub fn as_str(self) -> &'static str {
        match self {
            PendingKind::Element => "element",
            PendingKind::If => "if",
            PendingKind::Each => "each",
            PendingKind::Snippet => "snippet",
        }
    }
}

/// One thing that is open: what it is, its name (an element's tag), and the byte where it opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub kind: PendingKind,
    pub name: Option<String>,
    pub start: usize,
}

/// A source made whole: the part of it that is a complete token, and the closers of what is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Repaired {
    /// The repaired text: `source[..cut]` and the closers.
    pub text: String,
    /// How many bytes of the source `text` begins with. A span past it is in the closers.
    pub cut: usize,
    /// What is open in the source that arrived, outermost first, an open `{#each}` included.
    pub pending: Vec<Pending>,
    /// Whether an open `{#each}` was cut away from the text (it is shown when its `{/each}` arrives).
    pub withheld: bool,
    /// When the source ends inside a start tag whose name and first attributes have arrived: the repaired text with that tag closed after its last complete
    /// attribute, and how many bytes of the source it keeps. It is used only to report what is already wrong with the tag (see [`step`]), never to draw.
    pub eager: Option<(String, usize)>,
}

/// What the eager repair starts from: where it keeps the source to, that text, and what is open there.
type EagerBasis = (usize, String, Vec<(PendingKind, Option<String>)>);

/// An open construct while scanning.
struct Open {
    kind: PendingKind,
    name: Option<String>,
    start: usize,
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_name(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-'
}

/// From the `{` at `start`, the byte after its matching `}`, reading strings (with their escapes) and nested braces; `None` if the source ends first.
fn braced(bytes: &[u8], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = start;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i += 1;
                loop {
                    match bytes.get(i)? {
                        b'\\' => i += 2,
                        b'"' => break,
                        _ => i += 1,
                    }
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }

    None
}

/// From the `"` at `start`, the byte after the closing quote; `None` if the source ends first.
fn quoted(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start + 1;

    loop {
        match bytes.get(i)? {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
}

fn skip_space(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// How far a start tag that the source ends inside has got: its name when the name is complete (a space or more follows it), and the end of the last attribute
/// that is complete (the end of the name when none is).
struct Partial {
    name: Option<String>,
    kept_end: usize,
}

/// A start tag from the `<` at `start`: where it ends, whether it is self-closing, and its name. `Err` says how far it got if the source ends inside it.
fn start_tag(bytes: &[u8], start: usize) -> Result<(usize, bool, String), Partial> {
    let mut i = start + 1;
    let name_from = i;

    while i < bytes.len() && is_name(bytes[i]) {
        i += 1;
    }

    let name = String::from_utf8_lossy(&bytes[name_from..i]).into_owned();
    // The name is complete only when something follows it.
    let mut partial = Partial {
        name: None,
        kept_end: i,
    };

    if i == bytes.len() {
        return Err(partial);
    }

    partial.name = Some(name.clone());

    loop {
        i = skip_space(bytes, i);

        match bytes.get(i) {
            None => return Err(partial),
            Some(b'>') => return Ok((i + 1, false, name)),
            Some(b'/') => match bytes.get(i + 1) {
                None => return Err(partial),
                Some(b'>') => return Ok((i + 2, true, name)),
                Some(_) => i += 1, // a stray `/`: the checker reports it
            },
            Some(_) => {
                // An attribute or an event binding: a name, `=`, a string or a braced expression.
                let from = i;

                while i < bytes.len()
                    && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'_' | b'.' | b'-'))
                {
                    i += 1;
                }

                if i == from {
                    i += 1; // something that is not a name: the checker reports it
                    continue;
                }

                i = skip_space(bytes, i);

                if i == bytes.len() {
                    return Err(partial);
                }

                if bytes[i] != b'=' {
                    continue; // an attribute with no value: the checker reports it
                }

                i = skip_space(bytes, i + 1);

                match bytes.get(i) {
                    None => return Err(partial),
                    Some(b'"') => {
                        i = quoted(bytes, i).ok_or_else(|| Partial {
                            name: partial.name.clone(),
                            kept_end: partial.kept_end,
                        })?
                    }
                    Some(b'{') => {
                        i = braced(bytes, i).ok_or_else(|| Partial {
                            name: partial.name.clone(),
                            kept_end: partial.kept_end,
                        })?
                    }
                    Some(_) => {}
                }

                partial.kept_end = i; // this attribute is complete
            }
        }
    }
}

/// Appends the closers of what is open, innermost first.
fn closers<'a>(text: &mut String, open: impl Iterator<Item = (PendingKind, Option<&'a str>)>) {
    for (kind, name) in open {
        match kind {
            PendingKind::Element => {
                text.push_str("</");
                text.push_str(name.unwrap_or(""));
                text.push('>');
            }
            PendingKind::If => text.push_str("{/if}"),
            PendingKind::Each => text.push_str("{/each}"),
            PendingKind::Snippet => text.push_str("{/snippet}"),
        }
    }
}

/// Makes `source` whole: see the module's documentation.
pub fn repair(source: &str) -> Repaired {
    let bytes = source.as_bytes();
    let mut stack: Vec<Open> = Vec::new();
    let mut i = 0usize;
    // The end of the last complete token: where the source is cut if it ends in the middle of one.
    let mut cut = 0usize;
    let mut partial: Option<(usize, Partial)> = None;

    'scan: while i < bytes.len() {
        match bytes[i] {
            b'<' => {
                if bytes.get(i + 1) == Some(&b'/') {
                    // An end tag.
                    let Some(close) = bytes[i..].iter().position(|byte| *byte == b'>') else {
                        break 'scan;
                    };
                    let name = source[i + 2..i + close].trim();

                    if let Some(at) = stack.iter().rposition(|open| {
                        open.kind == PendingKind::Element && open.name.as_deref() == Some(name)
                    }) {
                        stack.truncate(at);
                    }

                    i += close + 1;
                } else if bytes.get(i + 1).is_some_and(|byte| is_name_start(*byte)) {
                    match start_tag(bytes, i) {
                        Err(found) => {
                            partial = Some((i, found));
                            break 'scan;
                        }
                        Ok((end, self_closing, name)) => {
                            if !self_closing {
                                stack.push(Open {
                                    kind: PendingKind::Element,
                                    name: Some(name),
                                    start: i,
                                });
                            }
                            i = end;
                        }
                    }
                } else if i + 1 == bytes.len() {
                    break 'scan; // a `<` that may yet become a tag
                } else {
                    i += 1; // a `<` that is not a tag: the checker reports it
                }
                cut = i;
            }
            b'{' => {
                let Some(&next) = bytes.get(i + 1) else {
                    break 'scan;
                };
                let Some(end) = braced(bytes, i) else {
                    break 'scan;
                };
                let header =
                    String::from_utf8_lossy(&bytes[(i + 2).min(end - 1)..end - 1]).into_owned();

                match next {
                    b'#' => {
                        let word: String = header
                            .chars()
                            .take_while(|c| c.is_ascii_alphabetic())
                            .collect();
                        let kind = match word.as_str() {
                            "if" => Some(PendingKind::If),
                            "each" => Some(PendingKind::Each),
                            "snippet" => Some(PendingKind::Snippet),
                            _ => None,
                        };

                        if let Some(kind) = kind {
                            stack.push(Open {
                                kind,
                                name: None,
                                start: i,
                            });
                        }
                    }
                    b'/' => {
                        let word: String = header
                            .chars()
                            .take_while(|c| c.is_ascii_alphabetic())
                            .collect();
                        let kind = match word.as_str() {
                            "if" => Some(PendingKind::If),
                            "each" => Some(PendingKind::Each),
                            "snippet" => Some(PendingKind::Snippet),
                            _ => None,
                        };

                        if let Some(at) =
                            kind.and_then(|kind| stack.iter().rposition(|open| open.kind == kind))
                        {
                            stack.truncate(at);
                        }
                    }
                    _ => {}
                }

                i = end;
                cut = i;
            }
            _ => {
                // Text, up to the next `<` or `{`. At the end of the source it is still growing, and it is kept: what is not inside something open is the document's own
                // text, which the checker reports whole.
                let end = bytes[i..]
                    .iter()
                    .position(|byte| matches!(byte, b'<' | b'{'))
                    .map_or(bytes.len(), |at| i + at);

                i = end;
                cut = i;
            }
        }
    }

    // The tag the source ends inside, closed after its last complete attribute, over what was open before it. Not when an {#each} is open (the preview withholds it, and
    // its body is checked when it arrives whole).
    let eager_basis: Option<EagerBasis> = partial.as_ref().and_then(|(at, found)| {
        let name = found.name.clone()?;

        if stack.iter().any(|open| open.kind == PendingKind::Each) {
            return None;
        }

        let mut open: Vec<(PendingKind, Option<String>)> = stack
            .iter()
            .map(|each| (each.kind, each.name.clone()))
            .collect();

        open.push((PendingKind::Element, Some(name)));

        Some((
            found.kept_end.max(*at),
            source[..found.kept_end.max(*at)].to_string(),
            open,
        ))
    });

    let pending: Vec<Pending> = stack
        .iter()
        .map(|open| Pending {
            kind: open.kind,
            name: open.name.clone(),
            start: open.start,
        })
        .collect();

    // An open {#each} is withheld: cut back to the first of them, and what is open is what was open there.
    let withheld = match stack.iter().position(|open| open.kind == PendingKind::Each) {
        Some(at) => {
            cut = stack[at].start;
            stack.truncate(at);
            true
        }
        None => false,
    };

    let mut text = source[..cut].to_string();

    closers(
        &mut text,
        stack
            .iter()
            .rev()
            .map(|open| (open.kind, open.name.as_deref())),
    );

    // A source with nothing complete in it has nothing to check.
    if text.trim().is_empty() {
        text.clear();
    }

    let eager = eager_basis.map(|(kept, mut text, open)| {
        text.push('>');
        closers(
            &mut text,
            open.iter()
                .rev()
                .map(|(kind, name)| (*kind, name.as_deref())),
        );

        (text, kept)
    });

    Repaired {
        text,
        cut,
        pending,
        withheld,
        eager,
    }
}

/// The codes that are final once the part of a start tag they are about has arrived, whatever comes after: a name that is not a component, a prop or an event the
/// component lacks, an attribute written twice, a reference to nothing in scope, a value of the wrong type. They are the ones reported from a start tag that is still
/// being written (the eager repair). `missing-required-prop` is not here: the prop may be the next attribute.
const FINAL_IN_A_PART: &[&str] = &[
    "unknown-component",
    "unknown-prop",
    "unknown-event",
    "duplicate-attribute",
    "duplicate-event-binding",
    "hyphenated-attribute-name",
    "unknown-reference",
    "unknown-command",
    "unknown-member",
    "unknown-field",
    "unknown-variant",
    "unknown-special-value",
    "type-mismatch",
    "no-common-type",
    "command-arity-mismatch",
    "command-outside-handler",
    "handler-not-command",
    "event-value-outside-handler",
    "event-has-no-payload",
    "number-literal-out-of-range",
];

/// What checking one prefix gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Step {
    pub repaired: Repaired,
    /// Every diagnostic standing against the repaired prefix, spans in the repaired text (one in the closers is moved to the end of the source that arrived).
    pub diagnostics: Vec<Diagnostic>,
    /// The template of the repaired prefix, exactly when it has no error: the preview, drawn as a fresh render.
    pub template: Option<mesh_template::Template>,
}

/// Checks the repaired prefix of `source` as the template of `model`'s component.
pub fn step(model: &Model, source: &str) -> Step {
    let repaired = repair(source);

    if repaired.text.is_empty() {
        return Step {
            repaired,
            diagnostics: Vec::new(),
            template: None,
        };
    }

    let compiled = check::template(&repaired.text, model);
    let cut = repaired.cut;
    let mut early: Vec<Diagnostic> = Vec::new();

    // A start tag still being written is checked as far as it has got, for what is final about it (the model hears about a wrong prop when the attribute is
    // complete, not when the tag is). It is never drawn.
    if let Some((text, kept)) = &repaired.eager {
        early = check::source(text, Some(model))
            .into_iter()
            .filter(|diagnostic| {
                FINAL_IN_A_PART.contains(&diagnostic.code.as_str())
                    && diagnostic.span.end_byte <= *kept
            })
            .collect();
    }

    let clamp = |span: Span| Span {
        start_byte: span.start_byte.min(cut),
        end_byte: span.end_byte.min(cut),
    };
    let mut seen: HashSet<Key> = compiled.diagnostics.iter().map(key_of).collect();

    early.retain(|diagnostic| seen.insert(key_of(diagnostic)));

    // A diagnostic in the closers is moved to the end of the source that was kept; one from the eager repair is already within the part of the tag it kept.
    let diagnostics: Vec<Diagnostic> = compiled
        .diagnostics
        .into_iter()
        .map(|mut diagnostic| {
            if diagnostic.span.end_byte > cut {
                diagnostic.span = clamp(diagnostic.span);
            }
            diagnostic
        })
        .chain(early)
        .collect();

    // The preview is nothing while any diagnostic stands, one reported early from a tag still being written included.
    let template = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == mesh_syntax::Severity::Error)
    {
        None
    } else {
        compiled.template
    };

    Step {
        repaired,
        diagnostics,
        template,
    }
}

type Key = (&'static str, usize, usize);

fn key_of(diagnostic: &Diagnostic) -> Key {
    (
        diagnostic.code.as_str(),
        diagnostic.span.start_byte,
        diagnostic.span.end_byte,
    )
}

/// A stream session over a [`Model`]: `push` chunks, `close` when they end.
pub struct Session<'m> {
    model: &'m Model,
    text: String,
    reported: HashSet<Key>,
}

/// What one `push` found.
#[derive(Debug, Clone, PartialEq)]
pub struct Update {
    /// The diagnostics this push added: each mistake is reported once, at the first prefix that holds it.
    pub new: Vec<Diagnostic>,
    /// The step the whole text so far gave: what stands, the template, what is open.
    pub step: Step,
}

impl<'m> Session<'m> {
    /// Opens a session that checks as the template of `model`'s component.
    pub fn open(model: &'m Model) -> Self {
        Session {
            model,
            text: String::new(),
            reported: HashSet::new(),
        }
    }

    /// The text pushed so far.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Adds `chunk` and checks the repaired prefix of everything pushed.
    pub fn push(&mut self, chunk: &str) -> Update {
        self.text.push_str(chunk);

        let step = step(self.model, &self.text);
        let new = step
            .diagnostics
            .iter()
            .filter(|diagnostic| self.reported.insert(key_of(diagnostic)))
            .cloned()
            .collect();

        Update { new, step }
    }

    /// Ends the session: exactly the batch check of the whole text, whatever was pushed (decision G4).
    pub fn close(self) -> Vec<Diagnostic> {
        check::source(&self.text, Some(self.model))
    }
}
