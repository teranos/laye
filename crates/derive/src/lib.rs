use libp2p_identity::Keypair;

const INFO_PREFIX: &[u8] = b"laye-derive/v1|";

#[derive(Debug, thiserror::Error)]
pub enum PhraseError {
    #[error("bip39 parse: {0}")]
    Parse(#[from] bip39::Error),
    #[error("phrase yields {got}-byte entropy; laye-derive requires 32 bytes (24 words)")]
    WrongEntropyLength { got: usize },
}

pub fn seed_to_phrase(seed: &[u8; 32]) -> String {
    let Ok(m) = bip39::Mnemonic::from_entropy(seed) else {
        unreachable!("bip39 Mnemonic::from_entropy accepts any 32-byte input");
    };
    m.to_string()
}

pub fn phrase_to_seed(phrase: &str) -> Result<[u8; 32], PhraseError> {
    let m = bip39::Mnemonic::parse(phrase)?;
    let (entropy, len) = m.to_entropy_array();
    if len != 32 {
        return Err(PhraseError::WrongEntropyLength { got: len });
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&entropy[..32]);
    Ok(out)
}

pub fn derive_ed25519(seed: &[u8; 32], purpose: &str) -> Keypair {
    let hkdf = hkdf::Hkdf::<sha2::Sha256>::new(None, seed);
    let mut info = Vec::with_capacity(INFO_PREFIX.len() + purpose.len());
    info.extend_from_slice(INFO_PREFIX);
    info.extend_from_slice(purpose.as_bytes());
    let mut secret = [0u8; 32];
    let Ok(()) = hkdf.expand(&info, &mut secret) else {
        unreachable!("HKDF-SHA256 expand to 32 bytes is bounded by 255*32; cannot fail");
    };
    let Ok(kp) = Keypair::ed25519_from_bytes(secret) else {
        unreachable!("Ed25519 keypair from any 32 bytes cannot fail");
    };
    kp
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn pk_bytes(kp: &libp2p_identity::Keypair) -> [u8; 32] {
        kp.public().try_into_ed25519().unwrap().to_bytes()
    }

    #[test]
    fn same_seed_and_purpose_yield_same_pubkey() {
        let seed = [0u8; 32];
        assert_eq!(
            pk_bytes(&derive_ed25519(&seed, "laye/rave")),
            pk_bytes(&derive_ed25519(&seed, "laye/rave")),
        );
    }

    #[test]
    fn different_purposes_yield_different_pubkeys() {
        let seed = [0u8; 32];
        assert_ne!(
            pk_bytes(&derive_ed25519(&seed, "laye/rave")),
            pk_bytes(&derive_ed25519(&seed, "laye/starter")),
        );
    }

    #[test]
    fn canonical_vector_zero_seed_laye_rave() {
        // Locks the derivation math: HKDF-SHA256(salt=None,
        // ikm=[0u8;32]).expand(b"laye-derive/v1|laye/rave", 32 bytes) →
        // Ed25519 keypair whose public key is the bytes below. Change
        // either the info-prefix or the hash and this test breaks, on
        // purpose — every derived pubkey in the wild would break too.
        let seed = [0u8; 32];
        assert_eq!(
            pk_bytes(&derive_ed25519(&seed, "laye/rave")),
            [
                0xa7, 0x4b, 0x15, 0x8c, 0xe4, 0x23, 0x35, 0x33, 0xae, 0xe6, 0x62, 0xd1, 0x89, 0x2d,
                0x6b, 0x95, 0x6f, 0xd5, 0xc7, 0xe5, 0x4c, 0x67, 0x92, 0x13, 0x3f, 0xc1, 0xe4, 0x2c,
                0xd1, 0x9e, 0x54, 0x29,
            ],
        );
    }

    #[test]
    fn phrase_seed_round_trip() {
        let seed = [42u8; 32];
        let phrase = seed_to_phrase(&seed);
        let recovered = phrase_to_seed(&phrase).unwrap();
        assert_eq!(seed, recovered);
    }

    #[test]
    fn phrase_from_zero_seed_is_24_words() {
        let phrase = seed_to_phrase(&[0u8; 32]);
        assert_eq!(phrase.split_whitespace().count(), 24);
    }

    #[test]
    fn canonical_bip39_vector_zero_entropy() {
        // Public BIP39 spec vector for 32 bytes of zero entropy.
        // This is the phrase every human sees when they mint a fresh
        // laye identity on a machine with no entropy (test env only).
        assert_eq!(
            seed_to_phrase(&[0u8; 32]),
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon art",
        );
    }

    #[test]
    fn invalid_phrase_errors() {
        assert!(phrase_to_seed("not a real bip39 phrase at all here nope nope").is_err());
    }

    #[test]
    fn twelve_word_phrase_is_rejected() {
        // 24 words = 32-byte entropy is our contract. 12-word phrases
        // (16-byte entropy) are valid BIP39 but the wrong shape for
        // laye-derive: they'd silently give half the seed material.
        let twelve = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        assert!(phrase_to_seed(twelve).is_err());
    }

    #[test]
    fn different_seeds_yield_different_pubkeys() {
        let seed_a = [0u8; 32];
        let mut seed_b = [0u8; 32];
        seed_b[0] = 1;
        assert_ne!(
            pk_bytes(&derive_ed25519(&seed_a, "laye/rave")),
            pk_bytes(&derive_ed25519(&seed_b, "laye/rave")),
        );
    }
}
