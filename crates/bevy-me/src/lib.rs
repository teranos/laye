use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;

pub use laye_me::{BindingClaim, Identity, SignedBinding};

#[derive(Resource, Default)]
pub struct IdentityRes(pub Option<Identity>);

pub struct IdentityPlugin;

impl Plugin for IdentityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(IdentityRes::default());
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
