//! The one canonical form, and the digest taken over it (ADR-024 §9.6, §4).
//!
//! A digest is only a pin if two processes computing it over equal values get the same bytes. JSON
//! does not give that for free: member order is unspecified, and a value that is equal as a map can
//! serialize differently depending on how it was built. So there is exactly one function here
//! ([`canonical_json`]), everything that hashes or signs goes through it, and no caller gets to
//! write its own "close enough" serialization — a second canonical form is a second artifact.

use serde::Serialize;
use sha2::Digest;

/// The canonical bytes of any serializable value: sorted object keys, no insignificant whitespace.
///
/// # Errors
/// The value's own serialization error, as a string — every type in this module serializes, so this
/// is a defect rather than a case to branch on.
pub fn canonical_json<T: Serialize>(value: &T) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(value).map_err(|e| format!("canonical form: {e}"))?;
    let mut out = Vec::new();
    write_canonical(&value, &mut out);
    Ok(out)
}

/// Write a JSON value with object keys in sorted order.
fn write_canonical(value: &serde_json::Value, out: &mut Vec<u8>) {
    match value {
        serde_json::Value::Object(map) => {
            // `serde_json`'s map is a BTreeMap unless the `preserve_order` feature is on; sorting
            // here anyway, because the property must not depend on a dependency's feature flag.
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push(b'{');
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(serde_json::to_string(key).unwrap_or_default().as_bytes());
                out.push(b':');
                write_canonical(&map[*key], out);
            }
            out.push(b'}');
        }
        serde_json::Value::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        other => out.extend_from_slice(serde_json::to_string(other).unwrap_or_default().as_bytes()),
    }
}

/// SHA-256, lowercase hex — the same encoding the audit chain uses for `prev_hash`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = sha2::Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The digest of a receipt, taken over its canonical form.
///
/// The newtype exists so a digest cannot be confused with any other hex string in a signature: a
/// `ReceiptDigest` is the only thing a sealed record and an attestation accept.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReceiptDigest(String);

impl ReceiptDigest {
    /// The digest of a value's canonical bytes.
    ///
    /// # Errors
    /// The value's canonical-form error.
    pub fn of<T: Serialize>(value: &T) -> Result<Self, String> {
        Ok(Self(sha256_hex(&canonical_json(value)?)))
    }

    /// From an already-computed hex digest, for verifying a record read back from the store.
    #[must_use]
    pub fn from_hex(hex: impl Into<String>) -> Self {
        Self(hex.into())
    }

    /// The hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ReceiptDigest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
