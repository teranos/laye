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

const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 12.0, 16.0);

pub const RELAY_MULTIADDR: &str =
    "/dns4/relaye.sbvh.nl/tcp/443/wss/p2p/12D3KooWC6UBnnmhhv3BAfYKyW1bFBD4GtC5waiEgQWJCb7Hbqaf";

pub const CHAT_TOPIC: &str = "rave-chat/v1";
pub const POSITIONS_TOPIC: &str = "rave-positions/v1";

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

pub fn build_and_run_app(_identity_bytes: Option<Vec<u8>>) {
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

    app.init_state::<AppState>();
    app.add_systems(Startup, setup_camera);
    app.add_systems(OnEnter(AppState::Login), (spawn_login_screen, spawn_login_orb));
    app.add_systems(OnExit(AppState::Login), (despawn_login_screen, despawn_login_orb));
    app.add_systems(
        Update,
        (on_login_pressed, spin_login_orb).run_if(in_state(AppState::Login)),
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
            ],
            identify_protocol: "/laye-starter/1.0.0".to_string(),
        });
        app.add_plugins(bevy_chat::ChatPlugin {
            topic: CHAT_TOPIC.to_string(),
            max_body_bytes: 512,
        });
        app.insert_resource(RemotePlayers::default());
        app.add_systems(
            Update,
            (
                publish_self_position,
                drain_position_events,
                render_remote_players,
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
                    Text::new("Log in"),
                    TextFont {
                        font_size: FontSize::Px(14.0),
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.9, 0.9)),
                ));
            });
        });
}

fn despawn_login_screen(mut commands: Commands, screens: Query<Entity, With<LoginScreen>>) {
    for e in &screens {
        commands.entity(e).despawn();
    }
}

fn on_login_pressed(
    q: Query<&Interaction, (Changed<Interaction>, With<LoginButton>)>,
    mut next: ResMut<NextState<AppState>>,
    mut identity: ResMut<IdentityRes>,
) {
    for i in &q {
        if *i == Interaction::Pressed {
            identity.0 = Some(Identity {
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
            next.set(AppState::InGame);
        }
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
