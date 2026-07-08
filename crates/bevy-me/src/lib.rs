use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

pub use laye_me::{BindingClaim, Identity, SignedBinding, SignedChat};

#[derive(Resource, Default)]
pub struct IdentityRes(pub Option<Identity>);

/// Per-remote-peer table of verified `SignedBinding`s the app has
/// observed on `laye-identity/v1`. Keyed by the peer's 32-byte
/// Ed25519 pubkey so a receiver can look up "who signed THIS
/// chat message" by handing over `SignedChat::author_peer_pubkey`
/// directly, without a PeerId ⇄ pubkey conversion.
#[derive(Resource, Default)]
pub struct BindingTable(pub std::collections::HashMap<[u8; 32], Vec<SignedBinding>>);

impl BindingTable {
    /// Resolve a display handle for a peer pubkey. Returns the
    /// first non-empty `handle` on the first binding for that
    /// peer — no provider ranking. Callers get the raw string
    /// (`"@onf@chaos.social"`) and format around it.
    pub fn resolve_handle(&self, peer_pubkey: &[u8; 32]) -> Option<&str> {
        self.0
            .get(peer_pubkey)?
            .iter()
            .find_map(|b| b.claim.handle.as_deref())
    }
}

pub struct IdentityPlugin;

impl Plugin for IdentityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IdentityRes::default());
        app.insert_resource(BindingTable::default());
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bevy_app::App;

    #[test]
    fn plugin_inserts_none_identity_by_default() {
        let mut app = App::new();
        app.add_plugins(IdentityPlugin);
        let res = app
            .world()
            .get_resource::<IdentityRes>()
            .expect("plugin inserts IdentityRes");
        assert!(res.0.is_none());
    }

    #[test]
    fn binding_table_resolve_handle_returns_first_available_handle() {
        let mut table = BindingTable::default();
        let pubkey = [0x21; 32];
        table.0.insert(
            pubkey,
            vec![
                SignedBinding {
                    claim: BindingClaim {
                        peer_pubkey: pubkey,
                        provider: "atproto".to_string(),
                        canonical_id: "did:plc:xxx".to_string(),
                        handle: None,
                        issued_at: 0,
                    },
                    signature: vec![],
                    signer_pubkey: [0u8; 32],
                },
                SignedBinding {
                    claim: BindingClaim {
                        peer_pubkey: pubkey,
                        provider: "mastodon".to_string(),
                        canonical_id: "https://chaos.social/@onf".to_string(),
                        handle: Some("@onf@chaos.social".to_string()),
                        issued_at: 1,
                    },
                    signature: vec![],
                    signer_pubkey: [0u8; 32],
                },
            ],
        );
        assert_eq!(table.resolve_handle(&pubkey), Some("@onf@chaos.social"));
    }

    #[test]
    fn binding_table_resolve_handle_returns_none_for_unseen_peer() {
        let table = BindingTable::default();
        assert_eq!(table.resolve_handle(&[0u8; 32]), None);
    }

    #[test]
    fn identity_res_can_be_mutated() {
        let mut app = App::new();
        app.add_plugins(IdentityPlugin);
        {
            let mut res = app.world_mut().resource_mut::<IdentityRes>();
            res.0 = Some(Identity {
                links: vec![SignedBinding {
                    claim: BindingClaim {
                        peer_pubkey: [0u8; 32],
                        provider: "test".to_string(),
                        canonical_id: "you".to_string(),
                        handle: Some("you".to_string()),
                        issued_at: 0,
                    },
                    signature: vec![],
                    signer_pubkey: [0u8; 32],
                }],
            });
        }
        let res = app.world().resource::<IdentityRes>();
        assert!(res.0.is_some());
    }
}
