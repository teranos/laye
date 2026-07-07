use bevy::asset::AssetMetaCheck;
use bevy::camera::Hdr;
use bevy::log::LogPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy_chat::ChatOverlayPlugin;
use bevy_drawer::{DrawerOverlayPlugin, DrawerPlugin};
use bevy_input_capture::{DefaultBindingsPlugin, InputCapture, InputCapturePlugin};
use bevy_me::{BindingClaim, Identity, IdentityPlugin, IdentityRes, SignedBinding};
use bevy_observability::{ErrorLog, ObservabilityPlugin, Severity};

#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
enum AppState {
    #[default]
    Login,
    InGame,
}

#[derive(Component)]
struct LoginScreen;

#[derive(Component)]
struct LoginButton;

#[derive(Component)]
struct LoginOrb;

#[derive(Resource)]
struct PeerPubkeyHex(pub Option<String>);

#[derive(Component)]
struct LoginError;

const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 12.0, 16.0);

pub const RELAY_MULTIADDR: &str =
    "/dns4/relaye.sbvh.nl/tcp/443/wss/p2p/12D3KooWC6UBnnmhhv3BAfYKyW1bFBD4GtC5waiEgQWJCb7Hbqaf";

pub const CHAT_TOPIC: &str = "rave-chat/v1";
pub const POSITIONS_TOPIC: &str = "rave-positions/v1";
pub const IDENTITY_TOPIC: &str = "laye-identity/v1";

#[derive(Component)]
struct Player;

#[cfg(target_arch = "wasm32")]
#[derive(serde::Serialize, serde::Deserialize)]
struct StarterPosition {
    peer: String,
    x: f32,
    y: f32,
    z: f32,
    at_ms: u64,
}

#[derive(Default)]
struct RemoteEntry {
    pos: Vec3,
    last_seen_ms: u64,
    entity: Option<Entity>,
}

#[derive(bevy::ecs::resource::Resource, Default)]
struct RemotePlayers(std::collections::HashMap<String, RemoteEntry>);

#[derive(Component)]
struct RemotePlayerCell;

#[derive(bevy::ecs::resource::Resource, Default)]
struct BindingTable(std::collections::HashMap<String, Vec<SignedBinding>>);

#[derive(bevy::ecs::resource::Resource, Default)]
struct BindingPublishAcc(f32);

pub fn build_and_run_app(
    _identity_bytes: Option<Vec<u8>>,
    peer_pubkey_hex: Option<String>,
) {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.02, 0.05)))
        .add_plugins((
            DefaultPlugins
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "bevy-starter".to_string(),
                        canvas: Some("#bevy".to_owned()),
                        fit_canvas_to_parent: true,
                        prevent_default_event_handling: false,
                        ..default()
                    }),
                    ..default()
                })
                .set(AssetPlugin {
                    meta_check: AssetMetaCheck::Never,
                    ..default()
                })
                .set(LogPlugin {
                    #[cfg(target_arch = "wasm32")]
                    custom_layer: crate::install_wasm_error_layer,
                    ..default()
                }),
            InputCapturePlugin,
            DefaultBindingsPlugin,
            ObservabilityPlugin,
            DrawerPlugin,
            DrawerOverlayPlugin,
            ChatOverlayPlugin {
                initial_history: vec![format!(
                    "{} · {}",
                    &env!("LAYE_COMMIT_SHA")[..7.min(env!("LAYE_COMMIT_SHA").len())],
                    env!("LAYE_BUILT_AT")
                )],
            },
            IdentityPlugin,
        ));

    app.insert_resource(PeerPubkeyHex(peer_pubkey_hex));
    app.init_state::<AppState>();
    app.add_systems(Startup, setup_camera);
    app.add_systems(OnEnter(AppState::Login), (spawn_login_screen, spawn_login_orb));
    app.add_systems(OnExit(AppState::Login), (despawn_login_screen, despawn_login_orb));
    app.add_systems(
        Update,
        (on_login_pressed, spin_login_orb, poll_login_result).run_if(in_state(AppState::Login)),
    );
    app.add_systems(OnEnter(AppState::InGame), setup_scene);

    #[cfg(target_arch = "wasm32")]
    {
        app.add_plugins(bevy_libp2p::LibP2PPlugin {
            bootstrap_addrs: vec![RELAY_MULTIADDR.to_string()],
            identity_bytes: _identity_bytes,
            topics: vec![
                bevy_libp2p::Topic(CHAT_TOPIC.to_string()),
                bevy_libp2p::Topic(POSITIONS_TOPIC.to_string()),
                bevy_libp2p::Topic(IDENTITY_TOPIC.to_string()),
            ],
            identify_protocol: "/laye-starter/1.0.0".to_string(),
        });
        app.add_plugins(bevy_chat::ChatPlugin {
            topic: CHAT_TOPIC.to_string(),
            max_body_bytes: 512,
        });
        app.insert_resource(RemotePlayers::default());
        app.insert_resource(BindingTable::default());
        app.insert_resource(BindingPublishAcc::default());
        app.add_systems(
            Update,
            (
                publish_self_position,
                drain_position_events,
                render_remote_players,
                publish_self_bindings,
                drain_identity_events,
            )
                .chain()
                .run_if(in_state(AppState::InGame)),
        );
    }

    app.add_systems(Startup, seed_drawer).add_systems(
        Update,
        (move_player_on_wasd, follow_player_with_camera)
            .chain()
            .run_if(in_state(AppState::InGame)),
    );
    app.run();
}

fn spawn_login_screen(mut commands: Commands) {
    commands
        .spawn((
            LoginScreen,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::axes(Val::Px(0.0), Val::Px(80.0)),
                ..default()
            },
        ))
        .with_children(|p| {
            p.spawn((
                Text::new("laye"),
                TextFont {
                    font_size: FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.92, 1.0)),
            ));
            p.spawn((
                LoginButton,
                Button,
                Node {
                    padding: UiRect::axes(Val::Px(24.0), Val::Px(10.0)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.1, 0.15, 0.2)),
            ))
            .with_children(|pp| {
                pp.spawn((
                    Text::new("Log in with Mastodon"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ));
            });
            p.spawn((
                LoginError,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.95, 0.5, 0.5)),
            ));
        });
}

fn despawn_login_screen(mut commands: Commands, screens: Query<Entity, With<LoginScreen>>) {
    for e in &screens {
        commands.entity(e).despawn();
    }
}

fn on_login_pressed(
    q: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
    peer: Res<PeerPubkeyHex>,
    mut errors: Query<&mut Text, With<LoginError>>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            let Some(peer_hex) = peer.0.as_ref() else {
                for mut t in &mut errors {
                    **t = "no peer pubkey — identity not loaded".to_string();
                }
                return;
            };
            for mut t in &mut errors {
                **t = String::new();
            }
            #[cfg(target_arch = "wasm32")]
            if let Err(msg) = crate::open_login_popup(peer_hex) {
                for mut t in &mut errors {
                    **t = msg.clone();
                }
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let _ = peer_hex;
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn decode_hex_32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, chunk) in s.as_bytes().chunks(2).enumerate() {
        out[i] = (hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?;
    }
    Some(out)
}

#[cfg(target_arch = "wasm32")]
fn decode_hex_variable(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        out.push((hex_nibble(chunk[0])? << 4) | hex_nibble(chunk[1])?);
    }
    Some(out)
}

#[cfg(target_arch = "wasm32")]
fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(10 + c - b'a'),
        b'A'..=b'F' => Some(10 + c - b'A'),
        _ => None,
    }
}

#[cfg(target_arch = "wasm32")]
fn poll_login_result(
    mut identity: ResMut<IdentityRes>,
    mut next: ResMut<NextState<AppState>>,
    mut errors: Query<&mut Text, With<LoginError>>,
) {
    let Some(outcome) = crate::take_login_outcome() else {
        return;
    };
    let signed = match outcome {
        crate::LoginOutcome::Error(msg) => {
            for mut t in &mut errors {
                **t = msg.clone();
            }
            return;
        }
        crate::LoginOutcome::Signed(s) => s,
    };
    let Some(peer_pk) = decode_hex_32(&signed.claim.peer_pubkey_hex) else {
        for mut t in &mut errors {
            **t = "peer_pubkey_hex not 32 bytes".to_string();
        }
        return;
    };
    let Some(signer_pk) = decode_hex_32(&signed.signer_pubkey_hex) else {
        for mut t in &mut errors {
            **t = "signer_pubkey_hex not 32 bytes".to_string();
        }
        return;
    };
    let Some(sig) = decode_hex_variable(&signed.signature_hex) else {
        for mut t in &mut errors {
            **t = "signature_hex not valid hex".to_string();
        }
        return;
    };
    identity.0 = Some(Identity {
        links: vec![SignedBinding {
            claim: BindingClaim {
                peer_pubkey: peer_pk,
                provider: signed.claim.provider,
                canonical_id: signed.claim.canonical_id,
                handle: signed.claim.handle,
                issued_at: signed.claim.issued_at,
            },
            signature: sig,
            signer_pubkey: signer_pk,
        }],
    });
    next.set(AppState::InGame);
}

#[cfg(not(target_arch = "wasm32"))]
fn poll_login_result(_identity: ResMut<IdentityRes>, _next: ResMut<NextState<AppState>>) {}

#[cfg(target_arch = "wasm32")]
fn publish_self_bindings(
    time: Res<Time>,
    mut acc: ResMut<BindingPublishAcc>,
    identity: Res<IdentityRes>,
    net: Res<bevy_libp2p::LayeNet>,
    mut log: ResMut<ErrorLog>,
) {
    acc.0 += time.delta_secs();
    if acc.0 < 5.0 {
        return;
    }
    acc.0 = 0.0;
    let Some(id) = identity.0.as_ref() else {
        return;
    };
    if id.links.is_empty() {
        return;
    }
    for binding in &id.links {
        let bytes = match serde_json::to_vec(binding) {
            Ok(b) => b,
            Err(e) => {
                log.emit(Severity::Warn, format!("bindings: serialize failed: {e}"));
                continue;
            }
        };
        if let Err(e) = net.publish(&bevy_libp2p::Topic(IDENTITY_TOPIC.to_string()), &bytes) {
            log.emit(Severity::Warn, format!("bindings: publish failed: {e}"));
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn drain_identity_events(
    net: Res<bevy_libp2p::LayeNet>,
    mut reader: MessageReader<bevy_libp2p::LibP2PMessage>,
    mut table: ResMut<BindingTable>,
    mut log: ResMut<ErrorLog>,
) {
    let self_peer = net.identity().0.clone();
    for msg in reader.read() {
        let bevy_libp2p::NetEvent::Message {
            topic,
            bytes,
            from,
            ..
        } = &msg.0
        else {
            continue;
        };
        if topic.0 != IDENTITY_TOPIC {
            continue;
        }
        if from.0 == self_peer {
            continue;
        }
        let binding: SignedBinding = match serde_json::from_slice(bytes) {
            Ok(b) => b,
            Err(e) => {
                log.emit(
                    Severity::Warn,
                    format!("bindings: parse from {}: {}", short_peer(&from.0), e),
                );
                continue;
            }
        };
        if let Err(e) = binding.verify() {
            log.emit(
                Severity::Warn,
                format!(
                    "bindings: verify failed from {}: {:?}",
                    short_peer(&from.0),
                    e
                ),
            );
            continue;
        }
        let entry = table.0.entry(from.0.clone()).or_default();
        let already = entry.iter().any(|b| {
            b.claim.provider == binding.claim.provider
                && b.claim.canonical_id == binding.claim.canonical_id
        });
        if !already {
            log.emit(
                Severity::Note,
                format!(
                    "bindings: {} = {} @ {}",
                    short_peer(&from.0),
                    binding.claim.handle.as_deref().unwrap_or("(no handle)"),
                    binding.claim.provider,
                ),
            );
            entry.push(binding);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn short_peer(p: &str) -> String {
    if p.len() > 10 {
        format!("{}…{}", &p[..6], &p[p.len() - 4..])
    } else {
        p.to_string()
    }
}

#[cfg(target_arch = "wasm32")]
fn publish_self_position(
    time: Res<Time>,
    mut acc: Local<f32>,
    players: Query<&Transform, With<Player>>,
    net: Res<bevy_libp2p::LayeNet>,
) {
    *acc += time.delta_secs();
    if *acc < 0.1 {
        return;
    }
    *acc = 0.0;
    let Some(tf) = players.iter().next() else {
        return;
    };
    let pos = StarterPosition {
        peer: net.identity().0.clone(),
        x: tf.translation.x,
        y: tf.translation.y,
        z: tf.translation.z,
        at_ms: js_sys::Date::now() as u64,
    };
    if let Ok(bytes) = serde_json::to_vec(&pos) {
        let _ = net.publish(&bevy_libp2p::Topic(POSITIONS_TOPIC.to_string()), &bytes);
    }
}

#[cfg(target_arch = "wasm32")]
fn drain_position_events(
    net: Res<bevy_libp2p::LayeNet>,
    mut reader: MessageReader<bevy_libp2p::LibP2PMessage>,
    mut remotes: ResMut<RemotePlayers>,
) {
    let self_peer = net.identity().0.clone();
    let now_ms = js_sys::Date::now() as u64;
    for msg in reader.read() {
        if let bevy_libp2p::NetEvent::Message { topic, bytes, .. } = &msg.0
            && topic.0 == POSITIONS_TOPIC
            && let Ok(pos) = serde_json::from_slice::<StarterPosition>(bytes)
            && pos.peer != self_peer
        {
            let entry = remotes.0.entry(pos.peer.clone()).or_default();
            entry.pos = Vec3::new(pos.x, pos.y, pos.z);
            entry.last_seen_ms = now_ms;
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn render_remote_players(
    mut commands: Commands,
    mut remotes: ResMut<RemotePlayers>,
    mut transforms: Query<&mut Transform, With<RemotePlayerCell>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let now_ms = js_sys::Date::now() as u64;
    let stale_cutoff = now_ms.saturating_sub(30_000);
    let stale_peers: Vec<String> = remotes
        .0
        .iter()
        .filter(|(_, e)| e.last_seen_ms < stale_cutoff)
        .map(|(p, _)| p.clone())
        .collect();
    for peer in stale_peers {
        if let Some(entry) = remotes.0.remove(&peer)
            && let Some(entity) = entry.entity
        {
            commands.entity(entity).despawn();
        }
    }
    for entry in remotes.0.values_mut() {
        match entry.entity {
            None => {
                let mesh = meshes.add(Sphere::new(0.6));
                let mat = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.9, 0.3, 0.85),
                    emissive: LinearRgba::rgb(1.4, 0.4, 1.2),
                    ..default()
                });
                let id = commands
                    .spawn((
                        Mesh3d(mesh),
                        MeshMaterial3d(mat),
                        Transform::from_translation(entry.pos),
                        RemotePlayerCell,
                    ))
                    .id();
                entry.entity = Some(id);
            }
            Some(entity) => {
                if let Ok(mut tf) = transforms.get_mut(entity) {
                    tf.translation = entry.pos;
                }
            }
        }
    }
}

fn seed_drawer(mut log: ResMut<ErrorLog>) {
    log.emit(Severity::Note, "bevy-starter booted");
    log.emit(Severity::Note, "press ` or \\ to toggle this drawer");
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Hdr,
        Bloom::default(),
        Transform::from_translation(CAMERA_OFFSET).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        DirectionalLight {
            illuminance: 2000.0,
            color: Color::srgb(0.6, 0.65, 0.8),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 20.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn spawn_login_orb(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let torus_mesh = meshes.add(Torus::new(2.0, 0.4));
    let torus_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.5, 0.9),
        emissive: LinearRgba::rgb(1.6, 2.2, 3.6),
        ..default()
    });
    commands.spawn((
        LoginOrb,
        Mesh3d(torus_mesh),
        MeshMaterial3d(torus_mat),
        Transform::from_xyz(0.0, 2.0, 0.0),
    ));
}

fn despawn_login_orb(mut commands: Commands, orbs: Query<Entity, With<LoginOrb>>) {
    for e in &orbs {
        commands.entity(e).despawn();
    }
}

fn spin_login_orb(time: Res<Time>, mut orbs: Query<&mut Transform, With<LoginOrb>>) {
    for mut t in &mut orbs {
        t.rotate_local_y(time.delta_secs() * 0.6);
        t.rotate_local_x(time.delta_secs() * 0.25);
    }
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let bowl_mesh = meshes.add(Cylinder::new(8.0, 0.4));
    let bowl_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.18, 0.25),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });
    commands.spawn((
        Mesh3d(bowl_mesh),
        MeshMaterial3d(bowl_mat),
        Transform::from_xyz(0.0, -0.2, 0.0),
    ));

    let player_mesh = meshes.add(Sphere::new(0.6));
    let player_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 0.9, 1.0),
        emissive: LinearRgba::rgb(0.8, 1.4, 2.0),
        ..default()
    });
    commands.spawn((
        Player,
        Mesh3d(player_mesh),
        MeshMaterial3d(player_mat),
        Transform::from_xyz(0.0, 0.6, 0.0),
    ));
}

fn move_player_on_wasd(
    keys: Res<ButtonInput<KeyCode>>,
    cap: Res<InputCapture>,
    time: Res<Time>,
    mut players: Query<&mut Transform, With<Player>>,
) {
    if cap.is_captured() {
        return;
    }
    let mut delta = Vec3::ZERO;
    if keys.pressed(KeyCode::KeyW) {
        delta.z -= 1.0;
    }
    if keys.pressed(KeyCode::KeyS) {
        delta.z += 1.0;
    }
    if keys.pressed(KeyCode::KeyA) {
        delta.x -= 1.0;
    }
    if keys.pressed(KeyCode::KeyD) {
        delta.x += 1.0;
    }
    if delta == Vec3::ZERO {
        return;
    }
    let step = delta.normalize() * 6.0 * time.delta_secs();
    for mut t in &mut players {
        t.translation += step;
    }
}

fn follow_player_with_camera(
    players: Query<&Transform, (With<Player>, Without<Camera3d>)>,
    mut cameras: Query<&mut Transform, (With<Camera3d>, Without<Player>)>,
) {
    let Ok(player) = players.single() else { return };
    for mut cam in &mut cameras {
        cam.translation = player.translation + CAMERA_OFFSET;
        cam.look_at(player.translation, Vec3::Y);
    }
}
