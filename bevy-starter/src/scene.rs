use bevy::asset::AssetMetaCheck;
use bevy::camera::Hdr;
use bevy::log::LogPlugin;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy_drawer::{DrawerOverlayPlugin, DrawerPlugin};
use bevy_input_capture::{DefaultBindingsPlugin, InputCapture, InputCapturePlugin};
use bevy_observability::{ErrorLog, ObservabilityPlugin, Severity};

const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 12.0, 16.0);

#[cfg(target_arch = "wasm32")]
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

#[cfg(target_arch = "wasm32")]
#[derive(Default)]
struct RemoteEntry {
    pos: Vec3,
    last_seen_ms: u64,
    entity: Option<Entity>,
}

#[cfg(target_arch = "wasm32")]
#[derive(bevy::ecs::resource::Resource, Default)]
struct RemotePlayers(std::collections::HashMap<String, RemoteEntry>);

#[cfg(target_arch = "wasm32")]
#[derive(Component)]
struct RemotePlayerCell;

pub fn build_and_run_app() {
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
        ));

    app.add_systems(Startup, (setup_camera, setup_scene, seed_drawer));
    #[cfg(target_arch = "wasm32")]
    {
        app.insert_resource(RemotePlayers::default());
        app.add_systems(Startup, subscribe_positions_topic);
        app.add_systems(
            Update,
            (publish_self_position, drain_position_events, render_remote_players).chain(),
        );
    }
    app.add_systems(
        Update,
        (move_player_on_wasd, follow_player_with_camera).chain(),
    );
    app.run();
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
    #[cfg(target_arch = "wasm32")]
    if crate::laye_extern::laye_is_focused() {
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

#[cfg(target_arch = "wasm32")]
fn subscribe_positions_topic() {
    // Fire-and-forget: laye-p2p surfaces its own errors in the sacred
    // red overlay. bevy-starter doesn't reinvent an error path.
    crate::laye_extern::subscribe_opaque(POSITIONS_TOPIC);
}

#[cfg(target_arch = "wasm32")]
fn publish_self_position(
    time: Res<Time>,
    mut acc: Local<f32>,
    players: Query<&Transform, With<Player>>,
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
        peer: crate::laye_extern::self_peer_id(),
        x: tf.translation.x,
        y: tf.translation.y,
        z: tf.translation.z,
        at_ms: js_sys::Date::now() as u64,
    };
    if let Ok(bytes) = serde_json::to_vec(&pos) {
        crate::laye_extern::publish(POSITIONS_TOPIC, bytes);
    }
}

#[cfg(target_arch = "wasm32")]
fn drain_position_events(mut remotes: ResMut<RemotePlayers>) {
    let n = crate::laye_extern::pending_bytes(POSITIONS_TOPIC);
    if n == 0 {
        return;
    }
    let buf = crate::laye_extern::recv_bytes(POSITIONS_TOPIC);
    let self_peer = crate::laye_extern::self_peer_id();
    let now_ms = js_sys::Date::now() as u64;
    for frame in crate::laye_extern::split_frames(&buf) {
        let Ok(pos) = serde_json::from_slice::<StarterPosition>(&frame) else {
            continue;
        };
        if pos.peer == self_peer {
            continue;
        }
        let entry = remotes.0.entry(pos.peer.clone()).or_default();
        entry.pos = Vec3::new(pos.x, pos.y, pos.z);
        entry.last_seen_ms = now_ms;
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

