//! Content-addressed digests: the trust variant key and the `solo.yml` content
//! hash for change detection.
//!
//! Both use SHA-256. The variant key is a *security* identifier — it answers "is
//! this exact command variant trusted on this machine?" — so it must be
//! collision-resistant: a fast non-cryptographic hash would let an untrusted
//! variant masquerade as a trusted one. The content hash lets sync skip work when
//! a file is touched but unchanged.

use std::fmt;

use sha2::{Digest, Sha256};

const HEX: &[u8; 16] = b"0123456789abcdef";

/// A 32-byte SHA-256 digest. Stored durably as lowercase hex.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; 32]);

impl Hash {
    /// Lowercase hex encoding (64 characters) — the durable, stored form.
    pub fn to_hex(&self) -> String {
        let mut s = String::with_capacity(64);
        for b in self.0 {
            s.push(HEX[(b >> 4) as usize] as char);
            s.push(HEX[(b & 0x0f) as usize] as char);
        }
        s
    }

    /// Parses a 64-character hex digest (either case).
    pub fn from_hex(s: &str) -> Result<Self, HashParseError> {
        if s.len() != 64 {
            return Err(HashParseError::BadLength(s.len()));
        }
        let mut buf = [0u8; 32];
        for (slot, pair) in buf.iter_mut().zip(s.as_bytes().chunks_exact(2)) {
            let hi = hex_val(pair[0]).ok_or(HashParseError::BadChar)?;
            let lo = hex_val(pair[1]).ok_or(HashParseError::BadChar)?;
            *slot = (hi << 4) | lo;
        }
        Ok(Self(buf))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

/// Errors parsing a hex digest back into a [`Hash`].
#[derive(Clone, Copy, PartialEq, Eq, Debug, thiserror::Error)]
pub enum HashParseError {
    #[error("hash must be 64 hex characters, got {0}")]
    BadLength(usize),
    #[error("hash contains a non-hex character")]
    BadChar,
}

/// An incremental, **length-prefixed** SHA-256 builder for hashing a structured
/// value out of several fields. The length prefix makes the encoding unambiguous:
/// the fields `("ab", "c")` and `("a", "bc")` produce different digests.
#[derive(Default)]
pub struct Hasher(Sha256);

impl Hasher {
    /// A fresh hasher.
    pub fn new() -> Self {
        Self(Sha256::new())
    }

    /// Mixes one length-delimited field into the digest.
    pub fn field(&mut self, bytes: &[u8]) -> &mut Self {
        self.0.update((bytes.len() as u64).to_le_bytes());
        self.0.update(bytes);
        self
    }

    /// Finalizes into a [`Hash`].
    pub fn finish(self) -> Hash {
        digest(self.0)
    }
}

/// SHA-256 of a single blob — the `solo.yml` content hash used for sync change
/// detection.
pub fn content_hash(bytes: &[u8]) -> Hash {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    digest(hasher)
}

fn digest(hasher: Sha256) -> Hash {
    let out = hasher.finalize();
    let mut buf = [0u8; 32];
    buf.copy_from_slice(&out);
    Hash(buf)
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trips() {
        let h = content_hash(b"soloist");
        let parsed = Hash::from_hex(&h.to_hex()).expect("valid hex");
        assert_eq!(h, parsed);
        assert_eq!(h.to_hex().len(), 64);
    }

    #[test]
    fn content_hash_is_stable_and_distinguishes_input() {
        assert_eq!(content_hash(b"same"), content_hash(b"same"));
        assert_ne!(content_hash(b"a"), content_hash(b"b"));
    }

    #[test]
    fn length_prefixing_disambiguates_field_boundaries() {
        let mut a = Hasher::new();
        a.field(b"ab").field(b"c");
        let mut b = Hasher::new();
        b.field(b"a").field(b"bc");
        assert_ne!(a.finish(), b.finish());
    }

    #[test]
    fn from_hex_rejects_bad_input() {
        assert_eq!(Hash::from_hex("abc"), Err(HashParseError::BadLength(3)));
        let non_hex = "z".repeat(64);
        assert_eq!(Hash::from_hex(&non_hex), Err(HashParseError::BadChar));
    }
}
