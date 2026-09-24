//! A JSON document tree with a source span on every value and key.
//!
//! `serde_json` does the parsing: it checks the syntax of the whole
//! document and hands back each value's exact source text as a borrowed
//! [`RawValue`]. A value's span is where that text sits in the document.
//! Objects keep every member in source order, duplicates included, so
//! the validator can report a duplicate key instead of silently keeping
//! the last one.

use mesh_syntax::Span;
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde_json::value::RawValue;
use std::fmt;

/// One JSON value and the span of its source text.
#[derive(Debug)]
pub(crate) struct Json {
    pub span: Span,
    pub value: Value,
}

#[derive(Debug)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Json>),
    Object(Vec<Member>),
}

/// One `"key": value` member of an object.
#[derive(Debug)]
pub(crate) struct Member {
    pub key: String,
    /// The key's source text, quotes included.
    pub key_span: Span,
    pub value: Json,
}

impl Value {
    /// The JSON type's name, for messages: "an object", "a string", ...
    pub fn describe(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "a boolean",
            Value::Number(_) => "a number",
            Value::String(_) => "a string",
            Value::Array(_) => "an array",
            Value::Object(_) => "an object",
        }
    }
}

/// Why a document isn't JSON: `serde_json`'s message, and where it
/// stopped.
#[derive(Debug)]
pub(crate) struct SyntaxError {
    pub message: String,
    pub span: Span,
}

/// Parses `source` into a spanned tree. A leading byte-order mark is
/// skipped; spans still index `source` as read, BOM included.
pub(crate) fn parse(source: &str) -> Result<Json, SyntaxError> {
    let body = source.strip_prefix('\u{feff}').unwrap_or(source);
    match serde_json::from_str::<&RawValue>(body) {
        Ok(raw) => Ok(node(source, raw)),
        // Reading into a `Value` finds the same mistake at the same place,
        // but names some more precisely: "trailing comma" rather than
        // "key must be a string".
        Err(raw_error) => {
            let error = serde_json::from_str::<serde_json::Value>(body)
                .err()
                .unwrap_or(raw_error);
            Err(syntax_error(source, body, &error))
        }
    }
}

fn node(source: &str, raw: &RawValue) -> Json {
    let text = raw.get();
    let span = span_of(source, text);
    // The whole document already parsed, so re-reading any part of it
    // can't fail.
    let value = match text.as_bytes().first() {
        Some(b'{') => {
            let Entries(entries) =
                serde_json::from_str(text).expect("an object inside a parsed document");
            let mut previous_end = span.start_byte + 1;
            let members = entries
                .into_iter()
                .map(|(key, raw)| {
                    let value = node(source, raw);
                    let key_span = key_span(source, previous_end, value.span.start_byte);
                    previous_end = value.span.end_byte;
                    Member {
                        key,
                        key_span,
                        value,
                    }
                })
                .collect();
            Value::Object(members)
        }
        Some(b'[') => {
            let items: Vec<&RawValue> =
                serde_json::from_str(text).expect("an array inside a parsed document");
            Value::Array(items.into_iter().map(|raw| node(source, raw)).collect())
        }
        _ => match serde_json::from_str(text).expect("a scalar inside a parsed document") {
            serde_json::Value::Null => Value::Null,
            serde_json::Value::Bool(value) => Value::Bool(value),
            serde_json::Value::Number(value) => Value::Number(value),
            serde_json::Value::String(value) => Value::String(value),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                unreachable!("text that doesn't start with `[` or `{{` is a scalar")
            }
        },
    };
    Json { span, value }
}

/// Where `text`, a slice borrowed from `source`, sits in `source`.
fn span_of(source: &str, text: &str) -> Span {
    let start_byte = text.as_ptr() as usize - source.as_ptr() as usize;
    Span {
        start_byte,
        end_byte: start_byte + text.len(),
    }
}

/// The span of the key between the end of the previous member (or the
/// object's `{`) and the start of this member's value. That gap holds
/// only whitespace, a `,`, the key string and a `:`, so the key runs from
/// the gap's first `"` to its last.
fn key_span(source: &str, gap_start: usize, gap_end: usize) -> Span {
    let gap = &source.as_bytes()[gap_start..gap_end];
    let open = gap
        .iter()
        .position(|&b| b == b'"')
        .expect("a key before every value");
    let close = gap
        .iter()
        .rposition(|&b| b == b'"')
        .expect("a key before every value");
    Span {
        start_byte: gap_start + open,
        end_byte: gap_start + close + 1,
    }
}

/// `serde_json`'s error, located at the byte it reports. Its line and
/// column count from 1, and the column counts bytes.
fn syntax_error(source: &str, body: &str, error: &serde_json::Error) -> SyntaxError {
    let line_start: usize = body
        .split_inclusive('\n')
        .take(error.line().saturating_sub(1))
        .map(str::len)
        .sum();
    let offset = (line_start + error.column().saturating_sub(1)).min(body.len());
    let offset = body.floor_char_boundary(offset) + (source.len() - body.len());
    let message = error.to_string();
    let message = match message.rsplit_once(" at line ") {
        Some((message, _)) => message.to_string(),
        None => message,
    };
    SyntaxError {
        message,
        span: Span {
            start_byte: offset,
            end_byte: offset,
        },
    }
}

/// An object's members in source order, duplicates included, each value
/// still raw.
struct Entries<'a>(Vec<(String, &'a RawValue)>);

impl<'de> Deserialize<'de> for Entries<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EntriesVisitor;

        impl<'de> Visitor<'de> for EntriesVisitor {
            type Value = Entries<'de>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut entries = Vec::new();
                while let Some(key) = map.next_key::<String>()? {
                    entries.push((key, map.next_value::<&'de RawValue>()?));
                }
                Ok(Entries(entries))
            }
        }

        deserializer.deserialize_map(EntriesVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slice(source: &str, span: Span) -> &str {
        &source[span.start_byte..span.end_byte]
    }

    #[test]
    fn spans_every_value_and_key() {
        let source = " {\n  \"a\" : [1, true, null],\n  \"b\\\"c\": {\"d\": \"é\"}\n}\n";
        let json = parse(source).expect("valid JSON");
        assert_eq!(slice(source, json.span).chars().next(), Some('{'));
        let Value::Object(members) = &json.value else {
            panic!("expected an object");
        };
        assert_eq!(slice(source, members[0].key_span), "\"a\"");
        assert_eq!(slice(source, members[0].value.span), "[1, true, null]");
        let Value::Array(items) = &members[0].value.value else {
            panic!("expected an array");
        };
        assert_eq!(slice(source, items[1].span), "true");
        assert_eq!(members[1].key, "b\"c");
        assert_eq!(slice(source, members[1].key_span), "\"b\\\"c\"");
        let Value::Object(inner) = &members[1].value.value else {
            panic!("expected an object");
        };
        assert_eq!(slice(source, inner[0].value.span), "\"é\"");
    }

    #[test]
    fn keeps_duplicate_keys_in_order() {
        let source = r#"{"a": 1, "a": 2}"#;
        let Value::Object(members) = parse(source).expect("valid JSON").value else {
            panic!("expected an object");
        };
        assert_eq!(members.len(), 2);
        assert_eq!(slice(source, members[1].key_span), "\"a\"");
        assert_eq!(members[1].key_span.start_byte, 9);
    }

    #[test]
    fn skips_a_byte_order_mark_but_keeps_raw_offsets() {
        let source = "\u{feff}{\"a\": 1}";
        let Value::Object(members) = parse(source).expect("valid JSON").value else {
            panic!("expected an object");
        };
        assert_eq!(members[0].key_span.start_byte, 4);

        let error = parse("\u{feff}{\"a\" 1}").expect_err("invalid JSON");
        assert_eq!(error.span.start_byte, 3 + 5);
        assert_eq!(error.message, "expected `:`");
    }

    #[test]
    fn locates_syntax_errors() {
        let source = "{\n  \"a\": 1,\n}";
        let error = parse(source).expect_err("trailing comma");
        assert_eq!(error.message, "trailing comma");
        assert_eq!(
            slice(
                source,
                Span {
                    start_byte: error.span.start_byte,
                    end_byte: error.span.start_byte + 1
                }
            ),
            "}"
        );

        let error = parse("").expect_err("empty");
        assert_eq!(error.message, "EOF while parsing a value");
        assert_eq!(error.span.start_byte, 0);

        let error = parse("{} x").expect_err("trailing characters");
        assert_eq!(error.message, "trailing characters");
        assert_eq!(error.span.start_byte, 3);
    }

    #[test]
    fn spans_exclude_whitespace_around_separators() {
        let source = "{\"a\"\t:\r\n  1 \t,\n\"b\" :{ \"c\" : [ 1 , 2 ] }  }";
        let Value::Object(members) = parse(source).expect("valid JSON").value else {
            panic!("expected an object");
        };
        assert_eq!(slice(source, members[0].key_span), "\"a\"");
        assert_eq!(slice(source, members[0].value.span), "1");
        assert_eq!(slice(source, members[1].key_span), "\"b\"");
        assert_eq!(
            slice(source, members[1].value.span),
            "{ \"c\" : [ 1 , 2 ] }"
        );
        let Value::Object(inner) = &members[1].value.value else {
            panic!("expected an object");
        };
        assert_eq!(slice(source, inner[0].key_span), "\"c\"");
        let Value::Array(items) = &inner[0].value.value else {
            panic!("expected an array");
        };
        assert_eq!(slice(source, items[0].span), "1");
        assert_eq!(slice(source, items[1].span), "2");
    }

    #[test]
    fn spans_keys_with_escapes_and_multi_byte_text() {
        // The key's span is its raw text, escapes and all; `key` is
        // decoded.
        let source = "\u{feff}{\"日本\": \"é\", \"a\\u002db\\\"\": {\"ü\": null}}";
        let Value::Object(members) = parse(source).expect("valid JSON").value else {
            panic!("expected an object");
        };
        assert_eq!(members[0].key, "日本");
        assert_eq!(slice(source, members[0].key_span), "\"日本\"");
        assert_eq!(slice(source, members[0].value.span), "\"é\"");
        assert_eq!(members[1].key, "a-b\"");
        assert_eq!(slice(source, members[1].key_span), "\"a\\u002db\\\"\"");
        let Value::Object(inner) = &members[1].value.value else {
            panic!("expected an object");
        };
        assert_eq!(slice(source, inner[0].key_span), "\"ü\"");
        assert_eq!(slice(source, inner[0].value.span), "null");
    }

    #[test]
    fn locates_syntax_errors_by_byte_after_multi_byte_text() {
        // serde_json's column counts bytes, so the offset lands on the
        // offending byte even after multi-byte characters and CRLF lines.
        let source = "{\r\n  \"日本\": \"é\"\r\n  x\r\n}";
        let error = parse(source).expect_err("missing comma");
        assert_eq!(error.message, "expected `,` or `}`");
        assert_eq!(&source[error.span.start_byte..], "x\r\n}");
        assert_eq!(error.span.start_byte, error.span.end_byte);
    }
}
