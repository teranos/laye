use bevy::prelude::*;
use bevy_input_capture::{Intent, IntentEvent};
use bevy_observability::{ErrorLog, Severity};

#[derive(Resource, Default)]
pub struct DrawerView {
    pub lines: Vec<String>,
    pub open: bool,
}

pub struct DrawerPlugin;

impl Plugin for DrawerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DrawerView {
            lines: Vec::new(),
            open: true,
        });
        app.add_systems(Update, refresh_view);
    }
}

fn refresh_view(log: Res<ErrorLog>, mut view: ResMut<DrawerView>) {
    view.lines = log
        .0
        .iter()
        .map(|e| format!("[{}] {}", severity_tag(e.severity), e.message))
        .collect();
}

fn severity_tag(s: Severity) -> &'static str {
    match s {
        Severity::Note => "note",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

#[derive(Component)]
struct DrawerOverlay;

#[derive(Component)]
struct DrawerText;

pub struct DrawerOverlayPlugin;

impl Plugin for DrawerOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_overlay);
        app.add_systems(Update, (toggle_on_intent, sync_visibility, sync_text).chain());
    }
}

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            DrawerOverlay,
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                width: Val::Percent(100.0),
                height: Val::Percent(40.0),
                padding: UiRect::all(Val::Px(12.0)),
                flex_direction: FlexDirection::Column,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
            Visibility::Hidden,
        ))
        .with_children(|p| {
            p.spawn((
                DrawerText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(11.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.85, 0.85)),
            ));
        });
}

fn toggle_on_intent(mut reader: MessageReader<IntentEvent>, mut view: ResMut<DrawerView>) {
    for IntentEvent(intent) in reader.read() {
        if *intent == Intent::DrawerToggle {
            view.open = !view.open;
        }
    }
}

fn sync_visibility(
    view: Res<DrawerView>,
    mut overlays: Query<&mut Visibility, With<DrawerOverlay>>,
) {
    let target = if view.open {
        Visibility::Inherited
    } else {
        Visibility::Hidden
    };
    for mut v in &mut overlays {
        if *v != target {
            *v = target;
        }
    }
}

fn sync_text(view: Res<DrawerView>, mut texts: Query<&mut Text, With<DrawerText>>) {
    if !view.is_changed() {
        return;
    }
    let body = if view.lines.is_empty() {
        "(no entries)".to_string()
    } else {
        view.lines.join("\n")
    };
    for mut t in &mut texts {
        if t.0 != body {
            t.0 = body.clone();
        }
    }
}
