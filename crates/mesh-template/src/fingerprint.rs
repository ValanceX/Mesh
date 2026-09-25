//! The model fingerprint (docs/manual/templates.md, "The model
//! fingerprint"): a digest of a manifest's meaning, and of nothing else.

use mesh_manifest::{Component, Manifest, Type};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

/// A model fingerprint: SHA-256, written `sha256:` and 64 lowercase
/// hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; 32]);

impl Fingerprint {
    /// The digest's bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("sha256:")?;
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

/// Why a string isn't a fingerprint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseFingerprintError;

impl fmt::Display for ParseFingerprintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a fingerprint is `sha256:` and 64 lowercase hexadecimal digits")
    }
}

impl std::error::Error for ParseFingerprintError {}

impl FromStr for Fingerprint {
    type Err = ParseFingerprintError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let hex = text.strip_prefix("sha256:").ok_or(ParseFingerprintError)?;
        if hex.len() != 64 {
            return Err(ParseFingerprintError);
        }
        let digit = |c: u8| match c {
            b'0'..=b'9' => Ok(c - b'0'),
            b'a'..=b'f' => Ok(c - b'a' + 10),
            _ => Err(ParseFingerprintError),
        };
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
            *byte = digit(pair[0])? << 4 | digit(pair[1])?;
        }
        Ok(Fingerprint(bytes))
    }
}

/// The fingerprint of `manifest`'s meaning: the manual's byte layout,
/// hashed with SHA-256.
///
/// Each named type is digested once and its digest reused, so the cost
/// is linear in the manifest's size however often a named type is used,
/// and the result equals what hashing the full expansion would give.
pub fn fingerprint(manifest: &Manifest) -> Fingerprint {
    let mut digests = Digests {
        manifest,
        named: HashMap::new(),
    };
    let mut hash = Sha256::new();
    string(&mut hash, "mesh-model-v1");
    count(&mut hash, manifest.components().len());
    // A `BTreeMap<String, _>` iterates in byte order, which for UTF-8 is
    // code-point order.
    for (name, component) in manifest.components() {
        string(&mut hash, name);
        hash.update(digests.component(component));
    }
    Fingerprint(hash.finalize().into())
}

struct Digests<'m> {
    manifest: &'m Manifest,
    named: HashMap<&'m str, [u8; 32]>,
}

impl<'m> Digests<'m> {
    /// `D(T)`.
    fn ty(&mut self, ty: &'m Type) -> [u8; 32] {
        let mut hash = Sha256::new();
        match ty {
            Type::Named(name) => {
                if let Some(digest) = self.named.get(name.as_str()) {
                    return *digest;
                }
                // A loaded manifest's named types resolve, and none is
                // recursive, so this terminates.
                let digest = self.ty(&self.manifest.types()[name]);
                self.named.insert(name, digest);
                return digest;
            }
            Type::String => hash.update([0x01]),
            Type::Number => hash.update([0x02]),
            Type::Boolean => hash.update([0x03]),
            Type::Null => hash.update([0x04]),
            Type::Any => hash.update([0x05]),
            Type::List(element) => {
                hash.update([0x06]);
                hash.update(self.ty(element));
            }
            Type::Optional(inner) => {
                hash.update([0x07]);
                hash.update(self.ty(inner));
            }
            Type::Record(fields) => {
                hash.update([0x08]);
                count(&mut hash, fields.len());
                for (name, field) in fields {
                    string(&mut hash, name);
                    flag(&mut hash, field.required);
                    hash.update(self.ty(&field.ty));
                }
            }
        }
        hash.finalize().into()
    }

    /// `C`.
    fn component(&mut self, component: &'m Component) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update([0x10]);
        count(&mut hash, component.props.len());
        for (name, prop) in &component.props {
            string(&mut hash, name);
            flag(&mut hash, prop.required);
            hash.update(self.ty(&prop.ty));
        }
        count(&mut hash, component.events.len());
        for (name, event) in &component.events {
            string(&mut hash, name);
            match &event.payload {
                None => hash.update([0x00]),
                Some(payload) => {
                    hash.update([0x01]);
                    hash.update(self.ty(payload));
                }
            }
        }
        count(&mut hash, component.commands.len());
        for (name, command) in &component.commands {
            string(&mut hash, name);
            count(&mut hash, command.parameters.len());
            for parameter in &command.parameters {
                hash.update(self.ty(&parameter.ty));
            }
        }
        count(&mut hash, component.scope.len());
        for (name, ty) in &component.scope {
            string(&mut hash, name);
            hash.update(self.ty(ty));
        }
        hash.finalize().into()
    }
}

/// A count: 32-bit big-endian.
fn count(hash: &mut Sha256, count: usize) {
    let count = u32::try_from(count).expect("a manifest has fewer than 2^32 declarations");
    hash.update(count.to_be_bytes());
}

/// A string: its UTF-8 byte length as a count, then its bytes.
fn string(hash: &mut Sha256, text: &str) {
    count(hash, text.len());
    hash.update(text.as_bytes());
}

/// A flag: one byte, `0x01` for true.
fn flag(hash: &mut Sha256, value: bool) {
    hash.update([u8::from(value)]);
}
