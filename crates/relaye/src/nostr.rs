//! Nostr login for the laye identity broker (NIP-46, QR-driven).
//!
//! Broker is the NIP-46 client; user scans a `nostrconnect://` QR
//! with their signer app (Primal, Amber, nsec.app). No browser
//! extension required.
//!
//! Consulted:
//! - <https://nips.nostr.com/46> — NIP-46 spec
//! - <https://nips.nostr.com/1> — event id canonical serialization
//! - <https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki>
//!   — BIP340 schnorr (test vectors used below)
//! - <https://buttondown.com/nostrcompass/archive/nostr-compass-4/>
//! - <https://github.com/PrimalHQ/primal-android-app/releases>

use k256::schnorr::{Signature as SchnorrSignature, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

#[allow(dead_code)] // wired to broker page + relay client in later M2k slices
pub fn nostrconnect_uri(pubkey_hex: &str, relay: &str, secret: &str) -> String {
    let mut out = String::from("nostrconnect://");
    out.push_str(pubkey_hex);
    out.push_str("?relay=");
    percent_encode_into(&mut out, relay);
    out.push_str("&secret=");
    percent_encode_into(&mut out, secret);
    out
}

#[allow(dead_code)] // called by nostrconnect_uri once broker page lands
fn percent_encode_into(out: &mut String, s: &str) {
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
}

/// Derives a BIP340 x-only pubkey (hex, lowercase, 64 chars) from a
/// 32-byte secp256k1 secret. Used both for ephemeral per-flow keypairs
/// and for anywhere we need to project a secret to its x-only form.
#[allow(dead_code)] // consumed by relay client + broker page in M2kd/M2ke
pub fn x_only_pubkey_hex(secret: &[u8; 32]) -> Result<String, k256::schnorr::Error> {
    let sk = SigningKey::from_bytes(secret)?;
    let vk: VerifyingKey = *sk.verifying_key();
    Ok(hex::encode(vk.to_bytes()))
}

/// NIP-01 canonical serialization of the event fields hashed into the
/// event id. No whitespace, only the seven required escape sequences
/// applied to the content string (line break, quote, backslash, CR,
/// tab, backspace, form feed). Tags are already-serialized JSON.
#[allow(dead_code)] // used by event_id + M2kd relay client
pub fn canonical_event_serialization(
    pubkey_hex: &str,
    created_at: u64,
    kind: u32,
    tags_json: &str,
    content: &str,
) -> String {
    let mut out = String::with_capacity(content.len() + 128);
    out.push_str("[0,\"");
    out.push_str(pubkey_hex);
    out.push_str("\",");
    out.push_str(&created_at.to_string());
    out.push(',');
    out.push_str(&kind.to_string());
    out.push(',');
    out.push_str(tags_json);
    out.push_str(",\"");
    escape_nip01_into(&mut out, content);
    out.push_str("\"]");
    out
}

fn escape_nip01_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            _ => out.push(c),
        }
    }
}

/// sha256 of the NIP-01 canonical serialization — this is the Nostr
/// event id and also the message that BIP340 signs.
#[allow(dead_code)] // used by M2kd relay client
pub fn event_id(
    pubkey_hex: &str,
    created_at: u64,
    kind: u32,
    tags_json: &str,
    content: &str,
) -> [u8; 32] {
    let s = canonical_event_serialization(pubkey_hex, created_at, kind, tags_json, content);
    Sha256::digest(s.as_bytes()).into()
}

/// BIP340 schnorr verification. `sig_hex` is 64 bytes (128 hex chars),
/// `pubkey_hex` is the 32-byte x-only pubkey (64 hex chars), `msg` is
/// the 32-byte event id. Uses `verify_raw` so the caller-provided
/// event id is signed as-is — k256's trait `verify` would sha256 it
/// again, double-hashing what Nostr's spec already fixed.
#[allow(dead_code)] // used by M2kd sign_event response verifier
pub fn verify_schnorr(msg: &[u8; 32], sig_hex: &str, pubkey_hex: &str) -> bool {
    let Ok(pk_bytes) = hex::decode(pubkey_hex) else { return false; };
    let Ok(sig_bytes) = hex::decode(sig_hex) else { return false; };
    let Ok(vk) = VerifyingKey::from_bytes(&pk_bytes) else { return false; };
    let Ok(sig) = SchnorrSignature::try_from(sig_bytes.as_slice()) else { return false; };
    vk.verify_raw(msg, &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nostrconnect_uri_shape() {
        let pubkey_hex =
            "0000000000000000000000000000000000000000000000000000000000000001";
        let relay = "wss://relay.example.com";
        let secret = "abc123";
        let uri = nostrconnect_uri(pubkey_hex, relay, secret);
        assert_eq!(
            uri,
            "nostrconnect://0000000000000000000000000000000000000000000000000000000000000001?relay=wss%3A%2F%2Frelay.example.com&secret=abc123"
        );
    }

    /// BIP340 test vector 0: secret 0x…03 → x-only pubkey F930…36F9.
    /// <https://github.com/bitcoin/bips/blob/master/bip-0340/test-vectors.csv>
    #[test]
    fn x_only_pubkey_matches_bip340_vector() {
        let mut sk = [0u8; 32];
        sk[31] = 3;
        assert_eq!(
            x_only_pubkey_hex(&sk).unwrap(),
            "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
        );
    }

    #[test]
    fn canonical_serialization_matches_nip01_shape() {
        let s = canonical_event_serialization(
            "0000000000000000000000000000000000000000000000000000000000000001",
            1_700_000_000,
            24_133,
            r#"[["p","abcd"]]"#,
            "hello \"world\"\n",
        );
        assert_eq!(
            s,
            r#"[0,"0000000000000000000000000000000000000000000000000000000000000001",1700000000,24133,[["p","abcd"]],"hello \"world\"\n"]"#,
        );
    }

    #[test]
    fn event_id_is_sha256_of_canonical_serialization() {
        let pubkey = "0000000000000000000000000000000000000000000000000000000000000001";
        let created_at = 1_700_000_000;
        let kind = 1;
        let tags = "[]";
        let content = "hi";
        let s = canonical_event_serialization(pubkey, created_at, kind, tags, content);
        let expected: [u8; 32] = Sha256::digest(s.as_bytes()).into();
        assert_eq!(event_id(pubkey, created_at, kind, tags, content), expected);
    }

    /// BIP340 test vector 0: same secret 0x…03, msg = 32 zero bytes,
    /// aux_rand = 32 zero bytes → known signature.
    #[test]
    fn verify_schnorr_bip340_vector_0() {
        let pubkey = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
        let msg = [0u8; 32];
        let sig = "e907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0";
        assert!(verify_schnorr(&msg, sig, pubkey));
    }

    #[test]
    fn verify_schnorr_rejects_tampered_signature() {
        let pubkey = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
        let msg = [0u8; 32];
        // Flip one bit of the valid sig.
        let sig = "f907831f80848d1069a5371b402410364bdf1c5f8307b0084c55f1ce2dca821525f66a4a85ea8b71e482a74f382d2ce5ebeee8fdb2172f477df4900d310536c0";
        assert!(!verify_schnorr(&msg, sig, pubkey));
    }
}
