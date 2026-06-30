use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_input::ButtonInput;
use bevy_input::keyboard::KeyCode;

pub use laye_input::Intent;

#[derive(Resource, Default, Debug)]
pub struct InputCapture(pub laye_input::InputClaims);

impl InputCapture {
    pub fn claim(&mut self, who: &'static str) {
        self.0.claim(who);
    }
    pub fn release(&mut self, who: &'static str) {
        self.0.release(who);
    }
    pub fn release_all(&mut self) {
        self.0.release_all();
    }
    pub fn is_captured(&self) -> bool {
        self.0.is_captured()
    }
    pub fn claimants(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0.claimants()
    }
}

pub struct InputCapturePlugin;

impl Plugin for InputCapturePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(InputCapture::default());
    }
}

#[derive(Message, Debug, Clone, Copy)]
pub struct IntentEvent(pub Intent);

pub struct DefaultBindingsPlugin;

impl Plugin for DefaultBindingsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<IntentEvent>();
        app.add_systems(Update, (emit_default_intents, handle_release_all).chain());
    }
}

fn emit_default_intents(
    keys: Res<ButtonInput<KeyCode>>,
    mut writer: MessageWriter<IntentEvent>,
) {
    if keys.just_pressed(KeyCode::Escape) {
        writer.write(IntentEvent(Intent::ReleaseAll));
    }
    if keys.just_pressed(KeyCode::KeyT) {
        writer.write(IntentEvent(Intent::ChatFocus));
    }
    if keys.just_pressed(KeyCode::Backquote) || keys.just_pressed(KeyCode::Backslash) {
        writer.write(IntentEvent(Intent::DrawerToggle));
    }
    if keys.just_pressed(KeyCode::KeyP) {
        writer.write(IntentEvent(Intent::Screenshot));
    }
    if keys.just_pressed(KeyCode::Tab) {
        writer.write(IntentEvent(Intent::InventoryToggle));
    }
}

fn handle_release_all(
    mut reader: MessageReader<IntentEvent>,
    mut cap: ResMut<InputCapture>,
) {
    for IntentEvent(intent) in reader.read() {
        if *intent == Intent::ReleaseAll {
            cap.release_all();
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn input_capture_plugin_inserts_resource() {
        let mut app = App::new();
        app.add_plugins(InputCapturePlugin);
        let cap = app.world().get_resource::<InputCapture>().expect("plugin inserts InputCapture");
        assert!(!cap.is_captured());
    }

    #[test]
    fn input_capture_methods_delegate_to_laye_input() {
        let mut cap = InputCapture::default();
        cap.claim("chat");
        assert!(cap.is_captured());
        cap.release("chat");
        assert!(!cap.is_captured());
    }

    #[test]
    fn release_all_clears_capture_set() {
        let mut cap = InputCapture::default();
        cap.claim("chat");
        cap.claim("wardrobe");
        cap.release_all();
        assert!(!cap.is_captured());
    }
}
