//! Secret value object.
//!
//! @canonical .pi/architecture/modules/configuration.md#secret
//! Implements: Contract Freeze — Secret value object with redacted Debug/Display
//! Issue: #2
//!
//! API key wrapper that redacts its contents in all output channels
//! (Debug, Display) while allowing controlled access via `expose()`.
//!
//! # Contract (Frozen)
//! - `Secret::new()` wraps a string value
//! - `Secret::expose()` is the ONLY way to access the inner value
//! - Debug/Display always redact the value (never leak)
//! - Serialization is transparent (writes the actual value)
//! - Deserialization wraps the value in Secret
//! - PartialEq/Eq compare by inner value (not by identity)
//! - Hash is computed on the inner value

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// A sensitive value (e.g. API key) that is redacted in all text output.
///
/// # Security
/// - `Debug` and `Display` always show `[REDACTED]`
/// - Only `expose()` reveals the inner value
/// - Serde serialization is transparent (writes actual value)
/// - Serde deserialization wraps the string
///
/// # Example
/// ```ignore
/// let key = Secret::new("sk-ant-abc123");
/// assert_eq!(format!("{key:?}"), "[REDACTED]");
/// assert_eq!(key.expose(), "sk-ant-abc123");
/// ```
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    /// Wrap a value into Secret, protecting it from accidental exposure.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Access the inner value. This is the only way to read the secret.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Check if the secret is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

// --- Trait implementations ---

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "<empty>")
        } else {
            write!(f, "[REDACTED]")
        }
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Secret {}

impl std::hash::Hash for Secret {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T: Into<String>> From<T> for Secret {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}

// --- Serde support ---

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}
