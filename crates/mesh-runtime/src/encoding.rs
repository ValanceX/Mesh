//! The byte encoding a JavaScript host's inputs cross into WebAssembly
//! in (docs/manual/runtime.md, "The JavaScript encoding"), and its
//! decoding. The encoder, in `@valancex/mesh-runtime`, only walks values
//! and writes what it finds; everything MESH decides about them is
//! decided after decoding, by the same code that judges a native host's
//! values (I11).
//!
//! Little-endian throughout. A **value** is one tag byte, then its body:
//!
//! | Tag | Value | Body |
//! |---|---|---|
//! | `0` | an absent list element (a hole, or `undefined`) | none; only inside a list |
//! | `1` | `null` | none |
//! | `2`, `3` | `false`, `true` | none |
//! | `4` | a number | 8 bytes: the binary64 bits, as they are |
//! | `5` | a string | a *string body*: u32 count of UTF-16 code units, then the units |
//! | `6` | a list | u32 count, then each element, a value |
//! | `7` | a record | u32 count, then each field: its name as a string body, then its value |
//! | `8` | a value outside the data model | its kind, as a string body |
//!
//! A **text list** (templates) is a u32 count, then each text as a u32
//! byte length and its UTF-8 bytes.

use crate::value::{HostKey, HostRecord, HostValue};

/// How deep a decoded value may nest, as for JSON snapshots
/// ([`HostValue::from_json`]).
const MAX_DEPTH: usize = 128;

/// Why bytes couldn't be decoded. Never a judgement of what a value
/// holds: that is the runtime's, after decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingError {
    /// The bytes aren't the encoding: a fault in the host's encoder.
    Malformed(String),
    /// A value nests deeper than 128 levels, as for JSON snapshots.
    TooDeep,
    /// A snapshot that isn't a record: a snapshot is the root's scope.
    NotARecord,
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EncodingError::Malformed(why) => write!(f, "not the MESH value encoding: {why}"),
            EncodingError::TooDeep => write!(f, "a value nests deeper than {MAX_DEPTH} levels"),
            EncodingError::NotARecord => write!(f, "a snapshot is a record"),
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], EncodingError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|&end| end <= self.bytes.len())
            .ok_or_else(|| EncodingError::Malformed(format!("truncated at byte {}", self.at)))?;
        let taken = &self.bytes[self.at..end];
        self.at = end;
        Ok(taken)
    }

    fn u8(&mut self) -> Result<u8, EncodingError> {
        Ok(self.take(1)?[0])
    }

    fn u32(&mut self) -> Result<usize, EncodingError> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize)
    }

    /// A count of items at least `least` bytes each, checked against what
    /// is left, so a forged count can't make decoding allocate.
    fn count(&mut self, least: usize) -> Result<usize, EncodingError> {
        let count = self.u32()?;
        if count.saturating_mul(least) > self.bytes.len() - self.at {
            return Err(EncodingError::Malformed(format!(
                "a count of {count} at byte {} is more than the bytes left",
                self.at - 4
            )));
        }
        Ok(count)
    }

    /// A string body: UTF-16 code units, which are a `String` exactly when
    /// they pair every surrogate.
    fn string(&mut self) -> Result<Result<String, Vec<u16>>, EncodingError> {
        let count = self.count(2)?;
        let units: Vec<u16> = self
            .take(count * 2)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        Ok(String::from_utf16(&units).map_err(|_| units))
    }

    fn value(&mut self, depth: usize, in_list: bool) -> Result<Option<HostValue>, EncodingError> {
        if depth > MAX_DEPTH {
            return Err(EncodingError::TooDeep);
        }
        let at = self.at;
        Ok(Some(match self.u8()? {
            0 if in_list => return Ok(None),
            1 => HostValue::Null,
            2 => HostValue::Boolean(false),
            3 => HostValue::Boolean(true),
            4 => {
                let bytes = self.take(8)?;
                let mut bits = [0; 8];
                bits.copy_from_slice(bytes);
                HostValue::Number(f64::from_bits(u64::from_le_bytes(bits)))
            }
            5 => match self.string()? {
                Ok(text) => HostValue::String(text),
                Err(units) => HostValue::Utf16(units),
            },
            6 => {
                let count = self.count(1)?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.value(depth + 1, true)?);
                }
                HostValue::List(items)
            }
            7 => {
                let count = self.count(5)?;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    let key = match self.string()? {
                        Ok(text) => HostKey::Text(text),
                        Err(units) => HostKey::Utf16(units),
                    };
                    let value = self
                        .value(depth + 1, false)?
                        .expect("only a list element is absent");
                    fields.push((key, value));
                }
                HostValue::Record(HostRecord(fields))
            }
            8 => match self.string()? {
                Ok(kind) => HostValue::Unsupported(kind),
                Err(_) => {
                    return Err(EncodingError::Malformed(format!(
                        "a kind at byte {at} isn't text"
                    )))
                }
            },
            tag => return Err(EncodingError::Malformed(format!("tag {tag} at byte {at}"))),
        }))
    }

    fn end(&self) -> Result<(), EncodingError> {
        if self.at == self.bytes.len() {
            Ok(())
        } else {
            Err(EncodingError::Malformed(format!(
                "trailing bytes from byte {}",
                self.at
            )))
        }
    }
}

/// Decodes one value: a snapshot or a payload.
pub fn decode_value(bytes: &[u8]) -> Result<HostValue, EncodingError> {
    let mut reader = Reader { bytes, at: 0 };
    let value = reader
        .value(0, false)?
        .expect("only a list element is absent");
    reader.end()?;
    Ok(value)
}

/// Decodes a snapshot: one value, which must be a record, since a
/// snapshot is the root's scope.
pub fn decode_snapshot(bytes: &[u8]) -> Result<HostRecord, EncodingError> {
    match decode_value(bytes)? {
        HostValue::Record(record) => Ok(record),
        _ => Err(EncodingError::NotARecord),
    }
}

/// Decodes a text list: the program's templates.
pub fn decode_texts(bytes: &[u8]) -> Result<Vec<String>, EncodingError> {
    let mut reader = Reader { bytes, at: 0 };
    let count = reader.count(4)?;
    let mut texts = Vec::with_capacity(count);
    for index in 0..count {
        let len = reader.u32()?;
        let text = std::str::from_utf8(reader.take(len)?)
            .map_err(|_| EncodingError::Malformed(format!("text {index} isn't UTF-8")))?;
        texts.push(text.to_string());
    }
    reader.end()?;
    Ok(texts)
}

/// Encodes a text list, as [`decode_texts`] reads it. For native hosts
/// and tests that speak the encoding.
pub fn encode_texts(texts: &[&str]) -> Vec<u8> {
    let mut bytes = (texts.len() as u32).to_le_bytes().to_vec();
    for text in texts {
        bytes.extend((text.len() as u32).to_le_bytes());
        bytes.extend(text.as_bytes());
    }
    bytes
}

/// Encodes a value, as [`decode_value`] reads it and the JavaScript
/// encoder writes it. For native hosts and tests that speak the encoding.
pub fn encode_value(value: &HostValue) -> Vec<u8> {
    let mut bytes = Vec::new();
    write_value(&mut bytes, Some(value));
    bytes
}

fn write_units(bytes: &mut Vec<u8>, units: &[u16]) {
    bytes.extend((units.len() as u32).to_le_bytes());
    for unit in units {
        bytes.extend(unit.to_le_bytes());
    }
}

fn write_value(bytes: &mut Vec<u8>, value: Option<&HostValue>) {
    let Some(value) = value else {
        bytes.push(0);
        return;
    };
    match value {
        HostValue::Null => bytes.push(1),
        HostValue::Boolean(false) => bytes.push(2),
        HostValue::Boolean(true) => bytes.push(3),
        HostValue::Number(number) => {
            bytes.push(4);
            bytes.extend(number.to_bits().to_le_bytes());
        }
        HostValue::String(text) => {
            bytes.push(5);
            write_units(bytes, &text.encode_utf16().collect::<Vec<_>>());
        }
        HostValue::Utf16(units) => {
            bytes.push(5);
            write_units(bytes, units);
        }
        // JavaScript has no number too large for binary64: a JSON reader
        // rounds it to infinity, as this does.
        HostValue::OutOfRange(_) => {
            bytes.push(4);
            bytes.extend(f64::INFINITY.to_bits().to_le_bytes());
        }
        HostValue::List(items) => {
            bytes.push(6);
            bytes.extend((items.len() as u32).to_le_bytes());
            for item in items {
                write_value(bytes, item.as_ref());
            }
        }
        HostValue::Record(record) => {
            bytes.push(7);
            bytes.extend((record.0.len() as u32).to_le_bytes());
            for (key, value) in &record.0 {
                match key {
                    HostKey::Text(text) => {
                        write_units(bytes, &text.encode_utf16().collect::<Vec<_>>())
                    }
                    HostKey::Utf16(units) => write_units(bytes, units),
                }
                write_value(bytes, Some(value));
            }
        }
        HostValue::Unsupported(kind) => {
            bytes.push(8);
            write_units(bytes, &kind.encode_utf16().collect::<Vec<_>>());
        }
    }
}
