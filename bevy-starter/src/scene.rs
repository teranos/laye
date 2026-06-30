use bevy::asset::AssetMetaCheck;
use bevy::camera::Hdr;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use bevy::window::WindowPlugin;

pub fn build_and_run_app() {
    let mut app = App::new();
    app.insert_resource(ClearColor(Color::srgb(0.01, 0.02, 0.05)))
        .add_plugins(
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
        )
        .add_systems(Startup, setup_scene);
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
        Transform::from_xyz(0.0, 12.0, 16.0).looking_at(Vec3::ZERO, Vec3::Y),
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
        Mesh3d(player_mesh),
        MeshMaterial3d(player_mat),
        Transform::from_xyz(0.0, 0.6, 0.0),
    ));
}
