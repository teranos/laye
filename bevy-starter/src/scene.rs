use bevy::asset::AssetMetaCheck;
use bevy::camera::Hdr;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::WindowPlugin;
use bevy_chat::ChatOverlayPlugin;
use bevy_drawer::{DrawerOverlayPlugin, DrawerPlugin};
use bevy_input_capture::{DefaultBindingsPlugin, InputCapture, InputCapturePlugin};
use bevy_observability::{ErrorLog, ObservabilityPlugin, Severity};

const CAMERA_OFFSET: Vec3 = Vec3::new(0.0, 12.0, 16.0);

#[derive(Component)]
struct Player;

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
                }),
            InputCapturePlugin,
            DefaultBindingsPlugin,
            ObservabilityPlugin,
            DrawerPlugin,
            DrawerOverlayPlugin,
            ChatOverlayPlugin,
        ))
        .add_systems(Startup, (setup_scene, seed_drawer))
        .add_systems(Update, (move_player_on_wasd, follow_player_with_camera).chain());
    app.run();
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
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

    let bowl_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.15, 0.18, 0.25),
        perceptual_roughness: 0.9,
        metallic: 0.0,
        ..default()
    });
    let floor_mesh = meshes.add(Cylinder::new(8.0, 0.3));
    commands.spawn((
        Mesh3d(floor_mesh),
        MeshMaterial3d(bowl_mat.clone()),
        Transform::from_xyz(0.0, -0.15, 0.0),
    ));
    let rim_mesh = meshes.add(Torus {
        major_radius: 8.0,
        minor_radius: 0.35,
    });
    commands.spawn((
        Mesh3d(rim_mesh),
        MeshMaterial3d(bowl_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
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

fn seed_drawer(mut log: ResMut<ErrorLog>) {
    log.emit(Severity::Note, "bevy-starter booted");
    log.emit(Severity::Note, "press ` or \\ to toggle this drawer");
    log.emit(Severity::Warn, "chat overlay lands in S5");
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
