//! Nostr login for the laye identity broker (NIP-46, QR-driven).
//!
//! Broker is the NIP-46 client; user scans a `nostrconnect://` QR
//! with their signer app (Primal, Amber, nsec.app). No browser
//! extension required.
//!
//! Consulted:
//! - <https://nips.nostr.com/46> — NIP-46 spec
//! - <https://nips.nostr.com/1> — event id canonical serialization
//! - <https://nips.nostr.com/44> — NIP-44 v2 payload cipher
//! - <https://github.com/paulmillr/nip44> — reference impl + vectors
//! - <https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki>
//!   — BIP340 schnorr (test vectors used below)
//! - <https://buttondown.com/nostrcompass/archive/nostr-compass-4/>
//! - <https://github.com/PrimalHQ/primal-android-app/releases>

use base64::Engine;
use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use k256::schnorr::{Signature as SchnorrSignature, SigningKey, VerifyingKey};
use k256::{PublicKey, SecretKey};
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
#[allow(dead_code)] // used by event_id + M2ke relay client
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
#[allow(dead_code)] // used by M2ke relay client
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
#[allow(dead_code)] // used by M2ke sign_event response verifier
pub fn verify_schnorr(msg: &[u8; 32], sig_hex: &str, pubkey_hex: &str) -> bool {
    let Ok(pk_bytes) = hex::decode(pubkey_hex) else {
        return false;
    };
    let Ok(sig_bytes) = hex::decode(sig_hex) else {
        return false;
    };
    let Ok(vk) = VerifyingKey::from_bytes(&pk_bytes) else {
        return false;
    };
    let Ok(sig) = SchnorrSignature::try_from(sig_bytes.as_slice()) else {
        return false;
    };
    vk.verify_raw(msg, &sig).is_ok()
}

// ============================================================================
// NIP-44 v2 payload cipher — used to wrap NIP-46 request/response contents.
// ============================================================================

/// Derives the NIP-44 v2 conversation key between `our_secret` and the
/// counterparty identified by `their_x_only_pubkey_hex`. Result is
/// `HKDF-Extract(salt="nip44-v2", ikm=ECDH_x)` — 32 bytes.
#[allow(dead_code)] // consumed by nip44_encrypt/decrypt
pub fn nip44_conversation_key(
    our_secret: &[u8; 32],
    their_x_only_pubkey_hex: &str,
) -> Option<[u8; 32]> {
    let pk_bytes = hex::decode(their_x_only_pubkey_hex).ok()?;
    let vk = VerifyingKey::from_bytes(&pk_bytes).ok()?;
    let their_pk = PublicKey::from_affine(*vk.as_affine()).ok()?;
    let our_sk = SecretKey::from_slice(our_secret).ok()?;
    let shared = k256::elliptic_curve::ecdh::diffie_hellman(
        our_sk.to_nonzero_scalar(),
        their_pk.as_affine(),
    );
    let (prk, _hk) =
        Hkdf::<Sha256>::extract(Some(b"nip44-v2"), shared.raw_secret_bytes().as_slice());
    Some(prk.into())
}

/// NIP-44 v2 encrypt. `nonce` is 32 bytes — real callers pass random
/// bytes; test vectors pass fixed bytes. Returns the base64 payload
/// exactly as it lands on the wire in the `content` field of a
/// kind:24133 event.
#[allow(dead_code)] // consumed by M2ke relay client
pub fn nip44_encrypt(
    conversation_key: &[u8; 32],
    plaintext: &[u8],
    nonce: &[u8; 32],
) -> Option<String> {
    let (chacha_key, chacha_nonce, hmac_key) = nip44_derive_message_keys(conversation_key, nonce)?;

    let padded_len = calc_padded_len(plaintext.len());
    let mut buf = Vec::with_capacity(2 + padded_len);
    buf.extend_from_slice(&(plaintext.len() as u16).to_be_bytes());
    buf.extend_from_slice(plaintext);
    buf.resize(2 + padded_len, 0);

    let mut cipher = ChaCha20::new((&chacha_key).into(), (&chacha_nonce).into());
    cipher.apply_keystream(&mut buf);
    let ciphertext = buf;

    let mut mac = Hmac::<Sha256>::new_from_slice(&hmac_key).ok()?;
    mac.update(nonce);
    mac.update(&ciphertext);
    let mac_bytes = mac.finalize().into_bytes();

    let mut payload = Vec::with_capacity(1 + 32 + ciphertext.len() + 32);
    payload.push(0x02);
    payload.extend_from_slice(nonce);
    payload.extend_from_slice(&ciphertext);
    payload.extend_from_slice(&mac_bytes);
    Some(base64::engine::general_purpose::STANDARD.encode(&payload))
}

/// NIP-44 v2 decrypt. Constant-time MAC verify before ChaCha20
/// keystream is applied. Returns the plaintext bytes; None if the
/// payload is malformed or the MAC does not match.
#[allow(dead_code)] // consumed by M2ke relay client
pub fn nip44_decrypt(conversation_key: &[u8; 32], payload_b64: &str) -> Option<Vec<u8>> {
    let payload = base64::engine::general_purpose::STANDARD
        .decode(payload_b64)
        .ok()?;
    // min payload: 1 version + 32 nonce + 2 length-prefix + 32 min-ct + 32 mac
    if payload.len() < 99 || payload[0] != 0x02 {
        return None;
    }
    let nonce: [u8; 32] = payload[1..33].try_into().ok()?;
    let ct_end = payload.len() - 32;
    let ciphertext = &payload[33..ct_end];
    let mac_expected = &payload[ct_end..];

    let (chacha_key, chacha_nonce, hmac_key) = nip44_derive_message_keys(conversation_key, &nonce)?;

    let mut mac = Hmac::<Sha256>::new_from_slice(&hmac_key).ok()?;
    mac.update(&nonce);
    mac.update(ciphertext);
    mac.verify_slice(mac_expected).ok()?;

    let mut plaintext_padded = ciphertext.to_vec();
    let mut cipher = ChaCha20::new((&chacha_key).into(), (&chacha_nonce).into());
    cipher.apply_keystream(&mut plaintext_padded);

    if plaintext_padded.len() < 2 {
        return None;
    }
    let plaintext_len = u16::from_be_bytes([plaintext_padded[0], plaintext_padded[1]]) as usize;
    if 2 + plaintext_len > plaintext_padded.len() {
        return None;
    }
    Some(plaintext_padded[2..2 + plaintext_len].to_vec())
}

fn nip44_derive_message_keys(
    conversation_key: &[u8; 32],
    nonce: &[u8; 32],
) -> Option<([u8; 32], [u8; 12], [u8; 32])> {
    let hk = Hkdf::<Sha256>::from_prk(conversation_key).ok()?;
    let mut okm = [0u8; 76];
    hk.expand(nonce, &mut okm).ok()?;
    let chacha_key: [u8; 32] = okm[0..32].try_into().ok()?;
    let chacha_nonce: [u8; 12] = okm[32..44].try_into().ok()?;
    let hmac_key: [u8; 32] = okm[44..76].try_into().ok()?;
    Some((chacha_key, chacha_nonce, hmac_key))
}

/// NIP-44 v2 padding — smallest power-of-two chunk that fits, minimum
/// 32 bytes. Padding length is computed on the unpadded plaintext
/// length, so a 1-byte message encrypts to the same wire length as a
/// 32-byte message.
fn calc_padded_len(unpadded_len: usize) -> usize {
    if unpadded_len <= 32 {
        return 32;
    }
    let next_power = unpadded_len.next_power_of_two();
    let chunk = if next_power <= 256 { 32 } else { next_power / 8 };
    chunk * ((unpadded_len - 1) / chunk + 1)
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

    /// NIP-44 v2 conversation_key vector from paulmillr/nip44:
    /// v2.valid.get_conversation_key[0].
    #[test]
    fn nip44_conversation_key_paulmillr_vector() {
        let sec1 =
            hex::decode("315e59ff51cb9209768cf7da80791ddcaae56ac9775eb25b6dee1234bc5d2268")
                .unwrap();
        let sec1_arr: [u8; 32] = sec1.as_slice().try_into().unwrap();
        let pub2 = "c2f9d9948dc8c7c38321e4b85c8558872eafa0641cd269db76848a6073e69133";
        let ck = nip44_conversation_key(&sec1_arr, pub2).unwrap();
        assert_eq!(
            hex::encode(ck),
            "3dfef0ce2a4d80a25e7a328accf73448ef67096f65f79588e358d9a0eb9013f1"
        );
    }

    /// NIP-44 v2 encrypt vector from paulmillr/nip44:
    /// v2.valid.encrypt_decrypt[0]. sec1 = 0x…01, sec2 = 0x…02,
    /// nonce = 0x…01, plaintext = "a".
    #[test]
    fn nip44_encrypt_paulmillr_vector() {
        let ck =
            hex::decode("c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d")
                .unwrap();
        let ck: [u8; 32] = ck.as_slice().try_into().unwrap();
        let mut nonce = [0u8; 32];
        nonce[31] = 1;
        let payload = nip44_encrypt(&ck, b"a", &nonce).unwrap();
        assert_eq!(
            payload,
            "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb"
        );
    }

    #[test]
    fn nip44_decrypt_paulmillr_vector() {
        let ck =
            hex::decode("c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d")
                .unwrap();
        let ck: [u8; 32] = ck.as_slice().try_into().unwrap();
        let payload = "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb";
        let plaintext = nip44_decrypt(&ck, payload).unwrap();
        assert_eq!(plaintext, b"a");
    }

    /// Roundtrip on a plaintext that exercises the padding branch
    /// (>32 bytes so calc_padded_len picks a non-minimum chunk).
    #[test]
    fn nip44_roundtrip_across_padding_boundary() {
        let ck = [7u8; 32];
        let nonce = [9u8; 32];
        let plaintext = b"the quick brown fox jumps over the lazy dog and then some more text";
        let payload = nip44_encrypt(&ck, plaintext, &nonce).unwrap();
        let recovered = nip44_decrypt(&ck, &payload).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn nip44_decrypt_rejects_tampered_mac() {
        let ck = [7u8; 32];
        let nonce = [9u8; 32];
        let payload = nip44_encrypt(&ck, b"hello", &nonce).unwrap();
        // Flip the last base64 character — that lands in the MAC bytes.
        let mut bytes = payload.into_bytes();
        let last = bytes.len() - 2; // avoid trailing '=' padding
        bytes[last] = if bytes[last] == b'A' { b'B' } else { b'A' };
        let tampered = String::from_utf8(bytes).unwrap();
        assert!(nip44_decrypt(&ck, &tampered).is_none());
    }
}
