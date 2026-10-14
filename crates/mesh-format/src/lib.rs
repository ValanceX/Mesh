//! The MPRX formatter (Format Document, Format Selection, indentation on Enter).
//!
//! It changes **whitespace between tokens and nothing else**. Whitespace-only
//! text between the structural tokens of a template (tags, `{expressions}`,
//! `{#block}` markers) is not part of the template's meaning (spec §3: it is
//! formatting, omitted from the semantic model), so it is re-indented by the
//! nesting of elements and blocks. Whitespace inside a text that has any other
//! character in it is meaning (the spec preserves it verbatim) and is never
//! touched; nor is anything inside a `{…}`. The whitespace between a tag's
//! attributes is free, so a tag already written over several lines gets one
//! attribute per line, with its closing `>` or `/>` on a line of its own.
//!
//! [`format`](fn@format) checks its own work: the tokens of the result must be exactly
//! those of the source, or it returns [`Error::Unsafe`] and the source stands.
//! A source that does not parse is not formatted ([`Error::Syntax`]): the
//! structure the indentation follows is not known.

use mesh_parser::parse;

/// How one level of nesting is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Columns per level, when indenting with spaces.
    pub tab_size: usize,
    /// Tabs instead of spaces.
    pub use_tabs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            tab_size: 2,
            use_tabs: false,
        }
    }
}

impl Options {
    fn indent(&self, levels: usize) -> String {
        if self.use_tabs {
            "\t".repeat(levels)
        } else {
            " ".repeat(levels * self.tab_size)
        }
    }
}

/// Why a source was not formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The source has syntax errors: its structure is not known.
    Syntax,
    /// The formatted text would not have the source's tokens. A bug in the
    /// formatter, never an effect on the source.
    Unsafe,
}

/// One replacement of the source text, by byte offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub start: usize,
    pub end: usize,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Text with a character that is not whitespace: meaning, kept as written.
    Text,
    /// Whitespace-only text: formatting.
    Gap,
    Open,
    Close,
    /// `{expression}`.
    Expr,
    /// `{#if …}` / `{#each …}`.
    BlockOpen,
    /// `{:else if …}` / `{:else}`.
    BlockMid,
    /// `{/if}` / `{/each}`.
    BlockClose,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Kind,
    start: usize,
    end: usize,
    /// For a tag: the end of `<name` (or of `</name`).
    name_end: usize,
    /// For an opening tag: the attributes, as written, in order.
    pieces: Vec<(usize, usize)>,
    self_closing: bool,
}

impl Token {
    fn simple(kind: Kind, start: usize, end: usize) -> Token {
        Token {
            kind,
            start,
            end,
            name_end: start,
            pieces: Vec::new(),
            self_closing: false,
        }
    }
}

/// The offset just past the `}` that closes the `{` at `open`, strings and
/// nested braces respected; the end of the text if it is never closed.
fn braces_end(src: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut at = open;
    while at < src.len() {
        match src[at] {
            b'"' => {
                at += 1;
                while at < src.len() && src[at] != b'"' {
                    at += if src[at] == b'\\' { 2 } else { 1 };
                }
            }
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return at + 1;
                }
            }
            _ => {}
        }
        at += 1;
    }
    src.len()
}

fn string_end(src: &[u8], open: usize) -> usize {
    let mut at = open + 1;
    while at < src.len() && src[at] != b'"' {
        at += if src[at] == b'\\' { 2 } else { 1 };
    }
    (at + 1).min(src.len())
}

fn is_name_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic()
}

fn lex(source: &str) -> Vec<Token> {
    let src = source.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < src.len() {
        let start = at;
        let byte = src[at];
        if byte == b'<' && src.get(at + 1) == Some(&b'/') {
            let end = src[at..]
                .iter()
                .position(|&each| each == b'>')
                .map_or(src.len(), |found| at + found + 1);
            let mut token = Token::simple(Kind::Close, start, end);
            token.name_end = end;
            tokens.push(token);
            at = end;
        } else if byte == b'<' && src.get(at + 1).copied().is_some_and(is_name_start) {
            let (token, end) = lex_open(src, at);
            tokens.push(token);
            at = end;
        } else if byte == b'{' {
            let end = braces_end(src, at);
            let kind = match src.get(at + 1) {
                Some(b'#') => Kind::BlockOpen,
                Some(b':') => Kind::BlockMid,
                Some(b'/') => Kind::BlockClose,
                _ => Kind::Expr,
            };
            tokens.push(Token::simple(kind, start, end));
            at = end;
        } else {
            // Text, up to the next thing that is not text.
            at += 1;
            while at < src.len() {
                let next = src[at];
                let tag = next == b'<'
                    && src
                        .get(at + 1)
                        .copied()
                        .is_some_and(|after| after == b'/' || is_name_start(after));
                if tag || next == b'{' {
                    break;
                }
                at += 1;
            }
            let kind = if source[start..at].trim().is_empty() {
                Kind::Gap
            } else {
                Kind::Text
            };
            tokens.push(Token::simple(kind, start, at));
        }
    }
    tokens
}

fn lex_open(src: &[u8], start: usize) -> (Token, usize) {
    let mut at = start + 1;
    while at < src.len()
        && !src[at].is_ascii_whitespace()
        && src[at] != b'>'
        && !(src[at] == b'/' && src.get(at + 1) == Some(&b'>'))
    {
        at += 1;
    }
    let name_end = at;
    let mut pieces = Vec::new();
    let mut self_closing = false;
    let end = loop {
        while at < src.len() && src[at].is_ascii_whitespace() {
            at += 1;
        }
        match src.get(at) {
            None => break src.len(),
            Some(b'>') => break at + 1,
            Some(b'/') if src.get(at + 1) == Some(&b'>') => {
                self_closing = true;
                break at + 2;
            }
            _ => {}
        }
        let piece = at;
        while at < src.len() {
            match src[at] {
                b'{' => at = braces_end(src, at),
                b'"' => at = string_end(src, at),
                byte if byte.is_ascii_whitespace() || byte == b'>' => break,
                b'/' if src.get(at + 1) == Some(&b'>') => break,
                _ => at += 1,
            }
        }
        pieces.push((piece, at));
    };
    (
        Token {
            kind: Kind::Open,
            start,
            end,
            name_end,
            pieces,
            self_closing,
        },
        end,
    )
}

fn newlines(text: &str) -> usize {
    text.bytes().filter(|&byte| byte == b'\n').count()
}

/// The edits that format `source`, over the whole text.
fn edits(source: &str, options: &Options) -> Vec<Edit> {
    let tokens = lex(source);
    let mut edits = Vec::new();
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token.kind {
            Kind::Gap => {
                let text = &source[token.start..token.end];
                let replacement = match tokens.get(index + 1) {
                    None => Some("\n".to_string()),
                    Some(_) if index == 0 => Some(String::new()),
                    Some(next) => {
                        let level = match next.kind {
                            Kind::Close | Kind::BlockClose | Kind::BlockMid => {
                                depth.saturating_sub(1)
                            }
                            _ => depth,
                        };
                        match newlines(text) {
                            0 => None,
                            1 => Some(format!("\n{}", options.indent(level))),
                            _ => Some(format!("\n\n{}", options.indent(level))),
                        }
                    }
                };
                if let Some(replacement) = replacement {
                    if replacement != text {
                        edits.push(Edit {
                            start: token.start,
                            end: token.end,
                            text: replacement,
                        });
                    }
                }
            }
            Kind::Open => {
                if newlines(&source[token.name_end..token.end]) > 0 {
                    let mut text = String::new();
                    for &(from, to) in &token.pieces {
                        text.push('\n');
                        text.push_str(&options.indent(depth + 1));
                        text.push_str(&source[from..to]);
                    }
                    match (token.pieces.is_empty(), token.self_closing) {
                        (true, true) => text.push_str(" />"),
                        (true, false) => text.push('>'),
                        (false, closing) => {
                            text.push('\n');
                            text.push_str(&options.indent(depth));
                            text.push_str(if closing { "/>" } else { ">" });
                        }
                    }
                    if text != source[token.name_end..token.end] {
                        edits.push(Edit {
                            start: token.name_end,
                            end: token.end,
                            text,
                        });
                    }
                }
                if !token.self_closing {
                    depth += 1;
                }
            }
            Kind::BlockOpen => depth += 1,
            Kind::Close | Kind::BlockClose => depth = depth.saturating_sub(1),
            Kind::Text | Kind::Expr | Kind::BlockMid => {}
        }
    }
    edits
}

fn apply(source: &str, edits: &[Edit]) -> String {
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    for edit in edits {
        out.push_str(&source[at..edit.start]);
        out.push_str(&edit.text);
        at = edit.end;
    }
    out.push_str(&source[at..]);
    out
}

/// What the formatter must not change: every token but the whitespace-only
/// ones, and each tag as its name and attributes.
fn signature(source: &str) -> Vec<String> {
    lex(source)
        .iter()
        .filter(|token| token.kind != Kind::Gap)
        .map(|token| match token.kind {
            Kind::Open => {
                let mut parts = vec![source[token.start..token.name_end].to_string()];
                parts.extend(
                    token
                        .pieces
                        .iter()
                        .map(|&(from, to)| source[from..to].to_string()),
                );
                parts.push(if token.self_closing { "/>" } else { ">" }.to_string());
                parts.join("\u{0}")
            }
            _ => source[token.start..token.end].to_string(),
        })
        .collect()
}

fn checked(source: &str, edits: &[Edit]) -> Result<(), Error> {
    if signature(&apply(source, edits)) == signature(source) {
        Ok(())
    } else {
        Err(Error::Unsafe)
    }
}

fn gate(source: &str) -> Result<(), Error> {
    let parsed = parse(source);
    if parsed.errors.is_empty() && parsed.ast.is_some() {
        Ok(())
    } else {
        Err(Error::Syntax)
    }
}

/// The edits that format the whole of `source`. Empty when it is formatted.
pub fn format_edits(source: &str, options: &Options) -> Result<Vec<Edit>, Error> {
    gate(source)?;
    let edits = edits(source, options);
    checked(source, &edits)?;
    Ok(edits)
}

/// The edits that format the part of `source` from byte `start` to byte `end`
/// (a selection): those of the formatting of the whole that touch it.
pub fn format_range_edits(
    source: &str,
    start: usize,
    end: usize,
    options: &Options,
) -> Result<Vec<Edit>, Error> {
    let all = format_edits(source, options)?;
    Ok(all
        .into_iter()
        .filter(|edit| edit.start <= end && edit.end >= start)
        .collect())
}

/// `source`, formatted.
pub fn format(source: &str, options: &Options) -> Result<String, Error> {
    Ok(apply(source, &format_edits(source, options)?))
}

/// The kind of the first token on the line that starts at byte `line_start`,
/// looking from token `from`; `None` if the line has nothing on it.
fn first_on_line(source: &str, tokens: &[Token], from: usize, line_start: usize) -> Option<Kind> {
    for token in &tokens[from..] {
        if token.kind != Kind::Gap {
            return Some(token.kind);
        }
        if source[token.start.max(line_start)..token.end].contains('\n') {
            return None;
        }
    }
    None
}

/// The indentation the line that starts at byte `line_start` should have, as
/// the nesting of elements and blocks before it says: for the new line after
/// Enter. Works on a source that does not parse (it is being typed).
pub fn indent_for_line(source: &str, line_start: usize, options: &Options) -> String {
    let tokens = lex(source);
    let mut depth = 0usize;
    let level_of = |first: Option<Kind>, depth: usize| match first {
        Some(Kind::Close | Kind::BlockClose | Kind::BlockMid) => depth.saturating_sub(1),
        _ => depth,
    };
    for (index, token) in tokens.iter().enumerate() {
        if token.end > line_start && token.start < line_start {
            // The line starts inside this token.
            return match token.kind {
                Kind::Open => {
                    let rest = source[line_start..token.end]
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim_start();
                    let closes = rest.starts_with('>') || rest.starts_with("/>");
                    options.indent(if closes { depth } else { depth + 1 })
                }
                Kind::Gap => options.indent(level_of(
                    first_on_line(source, &tokens, index, line_start),
                    depth,
                )),
                _ => previous_indent(source, line_start),
            };
        }
        if token.start >= line_start {
            return options.indent(level_of(
                first_on_line(source, &tokens, index, line_start),
                depth,
            ));
        }
        match token.kind {
            Kind::Open if !token.self_closing => depth += 1,
            Kind::BlockOpen => depth += 1,
            Kind::Close | Kind::BlockClose => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    options.indent(depth)
}

fn previous_indent(source: &str, line_start: usize) -> String {
    source[..line_start]
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| {
            line.chars()
                .take_while(|each| each.is_whitespace())
                .collect()
        })
        .unwrap_or_default()
}
