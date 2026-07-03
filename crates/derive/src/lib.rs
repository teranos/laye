use libp2p_identity::Keypair;

const INFO_PREFIX: &[u8] = b"laye-derive/v1|";

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
