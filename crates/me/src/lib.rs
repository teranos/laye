pub use libp2p_identity::Keypair;
use libp2p_identity::DecodingError;

#[derive(Debug, thiserror::Error)]
pub enum MeError {
    #[error("keypair protobuf decode: {0}")]
    Decode(#[from] DecodingError),
    #[error("keypair protobuf encode: {0}")]
    Encode(String),
}

#[derive(Default, Debug, Clone)]
pub struct Identity {
    pub links: Vec<SignedBinding>,
}

#[derive(Debug, Clone)]
pub struct BindingClaim {
    pub peer_pubkey: [u8; 32],
    pub provider: String,
    pub canonical_id: String,
    pub handle: Option<String>,
    pub issued_at: u64,
}

#[derive(Debug, Clone)]
pub struct SignedBinding {
    pub claim: BindingClaim,
    pub signature: Vec<u8>,
    pub signer_pubkey: [u8; 32],
}

impl BindingClaim {
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let peer_hex = hex_lower(&self.peer_pubkey);
        let handle = self.handle.as_deref().unwrap_or("");
        format!(
            "laye-binding/v1|{}|{}|{}|{}|{}",
            peer_hex, self.provider, self.canonical_id, handle, self.issued_at,
        )
        .into_bytes()
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
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

    #[test]
    fn canonical_bytes_is_pipe_delimited_v1_format() {
        let claim = BindingClaim {
            peer_pubkey: [0xab; 32],
            provider: "mastodon".to_string(),
            canonical_id: "https://chaos.social/@onf".to_string(),
            handle: Some("@onf@chaos.social".to_string()),
            issued_at: 1_735_000_000,
        };
        assert_eq!(
            claim.canonical_bytes(),
            b"laye-binding/v1|abababababababababababababababababababababababababababababababab|mastodon|https://chaos.social/@onf|@onf@chaos.social|1735000000",
        );
    }

    #[test]
    fn canonical_bytes_empty_handle_renders_as_empty_string() {
        let claim = BindingClaim {
            peer_pubkey: [0u8; 32],
            provider: "atproto".to_string(),
            canonical_id: "did:plc:xyz".to_string(),
            handle: None,
            issued_at: 0,
        };
        let bytes = claim.canonical_bytes();
        let s = std::str::from_utf8(&bytes).expect("utf8");
        assert!(s.contains("|atproto|did:plc:xyz||0"));
    }

    #[test]
    fn ed25519_sign_then_verify_round_trips_the_claim() {
        let signer = fresh();
        let claim = BindingClaim {
            peer_pubkey: [0x11; 32],
            provider: "mastodon".to_string(),
            canonical_id: "https://chaos.social/@onf".to_string(),
            handle: Some("@onf@chaos.social".to_string()),
            issued_at: 1_735_000_000,
        };
        let canonical = claim.canonical_bytes();
        let sig = signer.sign(&canonical).expect("sign");
        assert!(signer.public().verify(&canonical, &sig));
    }

    #[test]
    fn tampered_canonical_bytes_fail_verification() {
        let signer = fresh();
        let claim = BindingClaim {
            peer_pubkey: [0x11; 32],
            provider: "mastodon".to_string(),
            canonical_id: "https://chaos.social/@onf".to_string(),
            handle: None,
            issued_at: 1_735_000_000,
        };
        let canonical = claim.canonical_bytes();
        let sig = signer.sign(&canonical).expect("sign");
        let mut tampered = canonical.clone();
        tampered[0] ^= 0xff;
        assert!(!signer.public().verify(&tampered, &sig));
    }
}
