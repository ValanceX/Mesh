//! Values: the host's, before validation, and the evaluator's.

use std::collections::BTreeMap;
use std::rc::Rc;

/// A value as a host supplies it, before validation (§9.8). It can hold
/// everything a host might send, including what the boundary refuses,
/// so that refusing it is the runtime's judgement, not the host's.
#[derive(Debug, Clone, PartialEq)]
pub enum HostValue {
    Null,
    Boolean(bool),
    /// Any binary64 value, NaN, the infinities and `-0` included.
    Number(f64),
    String(String),
    /// A string that isn't Unicode: UTF-16 code units with an unpaired
    /// surrogate.
    Utf16(Vec<u16>),
    /// A JSON number too large for binary64, as written.
    OutOfRange(String),
    /// `None` is an absent element: JavaScript's `undefined`, or a hole.
    List(Vec<Option<HostValue>>),
    Record(HostRecord),
    /// A value outside the boundary data model, with its kind (§9.8.6):
    /// `function`, `symbol`, `bigint`, `object` (with a constructor's
    /// name), or `cycle`.
    Unsupported(String),
}

/// A record as a host supplies it: its fields in the order given. An
/// absent field is simply not here.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HostRecord(pub Vec<(HostKey, HostValue)>);

/// A record field's name as a host supplies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostKey {
    Text(String),
    /// A name that isn't Unicode: UTF-16 code units with an unpaired
    /// surrogate.
    Utf16(Vec<u16>),
}

impl HostRecord {
    /// The value of the field named `name`, if the record has one.
    pub fn get(&self, name: &str) -> Option<&HostValue> {
        self.0.iter().find_map(|(key, value)| match key {
            HostKey::Text(text) if text == name => Some(value),
            _ => None,
        })
    }

    /// Reads a snapshot from JSON text (§9.8.5): an object. For a native
    /// host; see [`HostValue::from_json`].
    pub fn from_json(text: &str) -> Result<HostRecord, String> {
        match HostValue::from_json(text)? {
            HostValue::Record(record) => Ok(record),
            _ => Err("a snapshot is a JSON object".to_string()),
        }
    }
}

impl HostValue {
    /// Reads a value from JSON text, as §9.8.5 maps JSON: each number to
    /// its nearest binary64 value, a number too large for binary64 to
    /// [`HostValue::OutOfRange`], and a string or key with an escaped
    /// unpaired surrogate to its UTF-16 form, so the runtime can refuse
    /// them where they're read. Text that isn't JSON, a repeated key, or
    /// nesting deeper than 128 levels is an `Err` for the host.
    pub fn from_json(text: &str) -> Result<HostValue, String> {
        let mut reader = Json {
            source: text,
            text: text.as_bytes(),
            at: 0,
        };
        reader.space();
        let value = reader.value(0)?;
        reader.space();
        if reader.at != reader.text.len() {
            return Err(reader.error("more after the value"));
        }
        Ok(value)
    }
}

/// A small JSON reader that keeps what `serde_json` would refuse or lose:
/// out-of-range numbers and unpaired surrogates.
struct Json<'t> {
    source: &'t str,
    text: &'t [u8],
    /// Always at a character boundary of `source`.
    at: usize,
}

const MAX_DEPTH: usize = 128;

enum Text {
    Unicode(String),
    Utf16(Vec<u16>),
}

impl Json<'_> {
    fn error(&self, what: &str) -> String {
        format!("not JSON at byte {}: {what}", self.at)
    }

    fn space(&mut self) {
        while matches!(self.text.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn eat(&mut self, byte: u8) -> Result<(), String> {
        if self.text.get(self.at) == Some(&byte) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.error(&format!("expected {:?}", char::from(byte))))
        }
    }

    fn word(&mut self, word: &str) -> bool {
        if self.text[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<HostValue, String> {
        if depth >= MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        match self.text.get(self.at) {
            Some(b'{') => {
                self.at += 1;
                let mut fields: Vec<(HostKey, HostValue)> = Vec::new();
                self.space();
                if self.text.get(self.at) == Some(&b'}') {
                    self.at += 1;
                    return Ok(HostValue::Record(HostRecord(fields)));
                }
                loop {
                    self.space();
                    let key = match self.string()? {
                        Text::Unicode(text) => HostKey::Text(text),
                        Text::Utf16(units) => HostKey::Utf16(units),
                    };
                    if fields.iter().any(|(seen, _)| *seen == key) {
                        return Err(self.error("a repeated key"));
                    }
                    self.space();
                    self.eat(b':')?;
                    self.space();
                    let value = self.value(depth + 1)?;
                    fields.push((key, value));
                    self.space();
                    if self.text.get(self.at) == Some(&b',') {
                        self.at += 1;
                        continue;
                    }
                    self.eat(b'}')?;
                    return Ok(HostValue::Record(HostRecord(fields)));
                }
            }
            Some(b'[') => {
                self.at += 1;
                let mut elements = Vec::new();
                self.space();
                if self.text.get(self.at) == Some(&b']') {
                    self.at += 1;
                    return Ok(HostValue::List(elements));
                }
                loop {
                    self.space();
                    elements.push(Some(self.value(depth + 1)?));
                    self.space();
                    if self.text.get(self.at) == Some(&b',') {
                        self.at += 1;
                        continue;
                    }
                    self.eat(b']')?;
                    return Ok(HostValue::List(elements));
                }
            }
            Some(b'"') => Ok(match self.string()? {
                Text::Unicode(text) => HostValue::String(text),
                Text::Utf16(units) => HostValue::Utf16(units),
            }),
            Some(b't') if self.word("true") => Ok(HostValue::Boolean(true)),
            Some(b'f') if self.word("false") => Ok(HostValue::Boolean(false)),
            Some(b'n') if self.word("null") => Ok(HostValue::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error("expected a value")),
        }
    }

    fn number(&mut self) -> Result<HostValue, String> {
        let start = self.at;
        let digits = |reader: &mut Self| {
            let from = reader.at;
            while matches!(reader.text.get(reader.at), Some(b'0'..=b'9')) {
                reader.at += 1;
            }
            reader.at > from
        };
        if self.text.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        if self.text.get(self.at) == Some(&b'0') {
            self.at += 1;
        } else if !digits(self) {
            return Err(self.error("expected digits"));
        }
        if self.text.get(self.at) == Some(&b'.') {
            self.at += 1;
            if !digits(self) {
                return Err(self.error("expected digits after the point"));
            }
        }
        if matches!(self.text.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.text.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !digits(self) {
                return Err(self.error("expected exponent digits"));
            }
        }
        let text = std::str::from_utf8(&self.text[start..self.at]).expect("ASCII");
        // Rust's parse is correctly rounded, ties to even, and gives an
        // infinity exactly when the number is out of range.
        let value: f64 = text.parse().map_err(|_| self.error("a malformed number"))?;
        Ok(if value.is_infinite() {
            HostValue::OutOfRange(text.to_string())
        } else {
            HostValue::Number(value)
        })
    }

    fn string(&mut self) -> Result<Text, String> {
        self.eat(b'"')?;
        let mut units: Vec<u16> = Vec::new();
        loop {
            let Some(&byte) = self.text.get(self.at) else {
                return Err(self.error("an unterminated string"));
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(match String::from_utf16(&units) {
                        Ok(text) => Text::Unicode(text),
                        Err(_) => Text::Utf16(units),
                    });
                }
                b'\\' => {
                    self.at += 1;
                    let escape = self.text.get(self.at).copied();
                    self.at += 1;
                    match escape {
                        Some(b'"') => units.push(0x22),
                        Some(b'\\') => units.push(0x5c),
                        Some(b'/') => units.push(0x2f),
                        Some(b'b') => units.push(0x08),
                        Some(b'f') => units.push(0x0c),
                        Some(b'n') => units.push(0x0a),
                        Some(b'r') => units.push(0x0d),
                        Some(b't') => units.push(0x09),
                        Some(b'u') => {
                            let hex = self
                                .text
                                .get(self.at..self.at + 4)
                                .and_then(|hex| std::str::from_utf8(hex).ok())
                                .and_then(|hex| u16::from_str_radix(hex, 16).ok())
                                .ok_or_else(|| self.error("a malformed \\u escape"))?;
                            self.at += 4;
                            units.push(hex);
                        }
                        _ => return Err(self.error("an unknown escape")),
                    }
                }
                0x00..=0x1f => return Err(self.error("a control character in a string")),
                _ => {
                    let character = self.source[self.at..]
                        .chars()
                        .next()
                        .expect("`at` is at a character boundary, before the end");
                    let mut buffer = [0; 2];
                    units.extend_from_slice(character.encode_utf16(&mut buffer));
                    self.at += character.len_utf8();
                }
            }
        }
    }
}

/// A value inside evaluation (§9.7.1). A record never holds an absent
/// field; a list may hold absent elements (which can't leave, §9.8).
#[derive(Debug, Clone)]
pub(crate) enum Value {
    Absent,
    Null,
    Boolean(bool),
    Number(f64),
    String(Rc<str>),
    List(Rc<[Value]>),
    Record(Rc<BTreeMap<String, Value>>),
}

impl Value {
    /// §9.7.4's `==`: strict, structural, IEEE 754 for numbers.
    pub(crate) fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Absent, Value::Absent) | (Value::Null, Value::Null) => true,
            (Value::Boolean(a), Value::Boolean(b)) => a == b,
            #[allow(clippy::float_cmp)]
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::List(a), Value::List(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(a, b)| a.equals(b))
            }
            (Value::Record(a), Value::Record(b)) => {
                a.len() == b.len()
                    && a.iter()
                        .all(|(name, a)| b.get(name).is_some_and(|b| a.equals(b)))
            }
            _ => false,
        }
    }

    /// A short name of the value's kind, for messages.
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Value::Absent => "absent",
            Value::Null => "null",
            Value::Boolean(_) => "a boolean",
            Value::Number(_) => "a number",
            Value::String(_) => "a string",
            Value::List(_) => "a list",
            Value::Record(_) => "a record",
        }
    }
}
