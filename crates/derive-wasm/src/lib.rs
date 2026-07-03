use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn derive_ed25519_pubkey(seed: &[u8], purpose: &str) -> Result<Vec<u8>, String> {
    let seed = seed_32(seed)?;
    let kp = laye_derive::derive_ed25519(&seed, purpose);
    let Ok(ed) = kp.public().try_into_ed25519() else {
        unreachable!("derive_ed25519 always yields an Ed25519 public key");
    };
    Ok(ed.to_bytes().to_vec())
}

#[wasm_bindgen]
pub fn derive_ed25519_sign(seed: &[u8], purpose: &str, msg: &[u8]) -> Result<Vec<u8>, String> {
    let seed = seed_32(seed)?;
    let kp = laye_derive::derive_ed25519(&seed, purpose);
    let Ok(sig) = kp.sign(msg) else {
        unreachable!("Ed25519 signing is infallible for a well-formed keypair");
    };
    Ok(sig)
}

#[wasm_bindgen]
pub fn phrase_to_seed(phrase: &str) -> Result<Vec<u8>, String> {
    laye_derive::phrase_to_seed(phrase)
        .map(|s| s.to_vec())
        .map_err(|e| e.to_string())
}

#[wasm_bindgen]
pub fn seed_to_phrase(seed: &[u8]) -> Result<String, String> {
    let seed = seed_32(seed)?;
    Ok(laye_derive::seed_to_phrase(&seed))
}

fn seed_32(seed: &[u8]) -> Result<[u8; 32], String> {
    if seed.len() != 32 {
        return Err(format!("seed must be 32 bytes, got {}", seed.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(seed);
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn pubkey_via_shim_matches_derive_directly() {
        let seed = [0u8; 32];
        let via_shim = derive_ed25519_pubkey(&seed, "laye/rave").unwrap();
        let via_derive = laye_derive::derive_ed25519(&seed, "laye/rave")
            .public()
            .try_into_ed25519()
            .unwrap()
            .to_bytes()
            .to_vec();
        assert_eq!(via_shim, via_derive);
    }

    #[test]
    fn wrong_seed_length_errors() {
        assert!(derive_ed25519_pubkey(&[0u8; 31], "laye/rave").is_err());
        assert!(derive_ed25519_pubkey(&[0u8; 33], "laye/rave").is_err());
    }

    #[test]
    fn signing_is_deterministic_and_purpose_scoped() {
        let seed = [7u8; 32];
        let msg = b"hello broker";
        let a = derive_ed25519_sign(&seed, "laye/starter", msg).unwrap();
        let b = derive_ed25519_sign(&seed, "laye/starter", msg).unwrap();
        assert_eq!(a, b, "Ed25519 signing is deterministic");

        let other_purpose = derive_ed25519_sign(&seed, "laye/rave", msg).unwrap();
        assert_ne!(
            a, other_purpose,
            "different purpose derives a different key so signs differently"
        );
    }

    #[test]
    fn phrase_seed_shim_round_trip() {
        let seed = [3u8; 32];
        let phrase = seed_to_phrase(&seed).unwrap();
        let recovered = phrase_to_seed(&phrase).unwrap();
        assert_eq!(seed.to_vec(), recovered);
    }

    #[test]
    fn phrase_shim_surfaces_parse_error_as_string() {
        let err = phrase_to_seed("not a real bip39 phrase at all here nope nope").unwrap_err();
        assert!(!err.is_empty());
    }
}
