pub use libp2p_identity::Keypair;
use libp2p_identity::DecodingError;

const HKDF_INFO_PREFIX: &[u8] = b"laye-me/v1|";

#[derive(Debug, thiserror::Error)]
pub enum MeError {
    #[error("keypair protobuf decode: {0}")]
    Decode(#[from] DecodingError),
    #[error("keypair protobuf encode: {0}")]
    Encode(String),
}

pub enum Identity {
    Local(Keypair),
    External {
        provider: String,
        canonical_id: String,
        handle: Option<String>,
    },
}

impl Identity {
    pub fn to_keypair(&self) -> Keypair {
        match self {
            Identity::Local(kp) => kp.clone(),
            Identity::External {
                provider,
                canonical_id,
                ..
            } => derive_ed25519(canonical_id.as_bytes(), provider),
        }
    }
}

pub fn derive_ed25519(seed: &[u8], purpose: &str) -> Keypair {
    let hkdf = hkdf::Hkdf::<sha2::Sha256>::new(None, seed);
    let mut info = Vec::with_capacity(HKDF_INFO_PREFIX.len() + purpose.len());
    info.extend_from_slice(HKDF_INFO_PREFIX);
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

pub fn fresh() -> Keypair {
    Keypair::generate_ed25519()
}

pub fn load(bytes: &[u8]) -> Result<Keypair, MeError> {
    Keypair::from_protobuf_encoding(bytes).map_err(MeError::Decode)
}

pub fn to_bytes(keypair: &Keypair) -> Result<Vec<u8>, MeError> {
    keypair
        .to_protobuf_encoding()
        .map_err(|e| MeError::Encode(format!("{e}")))
}

pub fn load_or_fresh(bytes: Option<&[u8]>) -> Result<Keypair, MeError> {
    match bytes {
        Some(b) => load(b),
        None => Ok(fresh()),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use libp2p_identity::PeerId;

    #[test]
    fn fresh_keypair_has_ed25519_public_of_32_bytes() {
        let kp = fresh();
        let pk_bytes = kp
            .public()
            .try_into_ed25519()
            .expect("ed25519 public")
            .to_bytes();
        assert_eq!(pk_bytes.len(), 32);
    }

    #[test]
    fn fresh_round_trips_via_bytes() {
        let kp = fresh();
        let bytes = to_bytes(&kp).expect("encode");
        let restored = load(&bytes).expect("decode");
        assert_eq!(PeerId::from(kp.public()), PeerId::from(restored.public()));
    }

    #[test]
    fn corrupt_bytes_surface_as_decode_error() {
        let result = load(&[0xFF; 8]);
        assert!(matches!(result, Err(MeError::Decode(_))));
    }

    #[test]
    fn load_or_fresh_none_mints_fresh() {
        let kp = load_or_fresh(None).expect("fresh path");
        let _ = PeerId::from(kp.public());
    }

    #[test]
    fn load_or_fresh_some_restores_same_peer_id() {
        let kp = fresh();
        let bytes = to_bytes(&kp).expect("encode");
        let restored = load_or_fresh(Some(&bytes)).expect("restore");
        assert_eq!(PeerId::from(kp.public()), PeerId::from(restored.public()));
    }

    fn pk_bytes(kp: &Keypair) -> [u8; 32] {
        kp.public().try_into_ed25519().unwrap().to_bytes()
    }

    #[test]
    fn local_identity_yields_original_keypair() {
        let kp = fresh();
        let expected = pk_bytes(&kp);
        let id = Identity::Local(kp);
        assert_eq!(pk_bytes(&id.to_keypair()), expected);
    }

    #[test]
    fn external_identity_yields_deterministic_keypair() {
        let id = || Identity::External {
            provider: "test".to_string(),
            canonical_id: "you".to_string(),
            handle: None,
        };
        assert_eq!(pk_bytes(&id().to_keypair()), pk_bytes(&id().to_keypair()));
    }

    #[test]
    fn external_different_canonical_ids_yield_different_pubkeys() {
        let alice = Identity::External {
            provider: "test".to_string(),
            canonical_id: "alice".to_string(),
            handle: None,
        };
        let bob = Identity::External {
            provider: "test".to_string(),
            canonical_id: "bob".to_string(),
            handle: None,
        };
        assert_ne!(pk_bytes(&alice.to_keypair()), pk_bytes(&bob.to_keypair()));
    }

    #[test]
    fn external_different_providers_dont_collide() {
        let via_atproto = Identity::External {
            provider: "atproto".to_string(),
            canonical_id: "you".to_string(),
            handle: None,
        };
        let via_mastodon = Identity::External {
            provider: "mastodon".to_string(),
            canonical_id: "you".to_string(),
            handle: None,
        };
        assert_ne!(
            pk_bytes(&via_atproto.to_keypair()),
            pk_bytes(&via_mastodon.to_keypair()),
        );
    }

    #[test]
    fn canonical_vector_zero_seed_laye_rave() {
        // Locks HKDF-SHA256(salt=None, ikm=[0u8;32]).expand(
        //   b"laye-me/v1|laye/rave", 32) → Ed25519. Change the info
        // prefix or the hash and this breaks — every derived pubkey
        // in the wild would break the same way.
        let seed = [0u8; 32];
        assert_eq!(
            pk_bytes(&derive_ed25519(&seed, "laye/rave")),
            [
                0x79, 0x87, 0xe7, 0x41, 0x72, 0xf3, 0x39, 0x91, 0x42, 0x9a, 0xf2, 0xad, 0x43, 0x6b,
                0xf2, 0xd0, 0x5b, 0xb6, 0x9b, 0x3f, 0x32, 0x5e, 0x2f, 0xaf, 0xe8, 0x19, 0xa3, 0xc4,
                0x86, 0x5c, 0xfe, 0xe0,
            ],
        );
    }
}
