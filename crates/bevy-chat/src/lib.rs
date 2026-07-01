use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy_input_capture::{InputCapture, Intent, IntentEvent};
use bevy_libp2p::{LayeNet, LibP2PMessage, NetEvent, Topic};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const CHAT_CLAIM: &str = "chat";
pub const HISTORY_CAP: usize = 40;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMsg {
    pub peer: String,
    pub body: String,
    pub at_ms: u64,
}

#[derive(Resource)]
pub struct ChatConfig {
    pub topic: Topic,
    pub max_body_bytes: usize,
}

#[derive(Message, Debug, Clone)]
pub struct OutgoingChat(pub String);

#[derive(Message, Debug, Clone)]
pub struct IncomingChat(pub ChatMsg);

pub struct ChatPlugin {
    pub topic: String,
    pub max_body_bytes: usize,
}

impl Default for ChatPlugin {
    fn default() -> Self {
        Self {
            topic: "laye-chat/v1".to_string(),
            max_body_bytes: 512,
        }
    }
}

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChatConfig {
            topic: Topic(self.topic.clone()),
            max_body_bytes: self.max_body_bytes,
        });
        app.add_message::<OutgoingChat>();
        app.add_message::<IncomingChat>();
        app.add_systems(Update, (publish_outgoing, route_incoming));
    }
}

fn publish_outgoing(
    net: Res<LayeNet>,
    cfg: Res<ChatConfig>,
    mut reader: MessageReader<OutgoingChat>,
) {
    let self_peer = net.identity().0.clone();
    for OutgoingChat(body) in reader.read() {
        if body.is_empty() {
            continue;
        }
        let trimmed = trim_to_char_boundary(body, cfg.max_body_bytes);
        let msg = ChatMsg {
            peer: self_peer.clone(),
            body: trimmed,
            at_ms: now_ms(),
        };
        let Ok(bytes) = serde_json::to_vec(&msg) else {
            continue;
        };
        let _ = net.publish(&cfg.topic, &bytes);
    }
}

fn route_incoming(
    net: Res<LayeNet>,
    cfg: Res<ChatConfig>,
    mut reader: MessageReader<LibP2PMessage>,
    mut writer: MessageWriter<IncomingChat>,
) {
    let self_peer = net.identity().0.clone();
    for msg in reader.read() {
        let NetEvent::Message { topic, bytes, .. } = &msg.0 else {
            continue;
        };
        if topic.0 != cfg.topic.0 {
            continue;
        }
        let Ok(chat) = serde_json::from_slice::<ChatMsg>(bytes) else {
            continue;
        };
        if chat.peer == self_peer {
            continue;
        }
        writer.write(IncomingChat(chat));
    }
}

fn trim_to_char_boundary(body: &str, max_bytes: usize) -> String {
    if body.len() <= max_bytes {
        return body.to_string();
    }
    let mut end = max_bytes;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_string()
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[cfg(not(target_arch = "wasm32"))]
fn now_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct ChatEntry {
    pub who: String,
    pub body: String,
}

#[derive(Resource, Default)]
pub struct ChatOverlayState {
    pub buffer: String,
    pub history: VecDeque<ChatEntry>,
}

#[derive(Component)]
struct ChatOverlay;

#[derive(Component)]
struct ChatHistoryText;

#[derive(Component)]
struct ChatInputText;

pub struct ChatOverlayPlugin;

impl Plugin for ChatOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatOverlayState>();
        app.add_message::<OutgoingChat>();
        app.add_message::<IncomingChat>();
        app.add_systems(Startup, spawn_overlay);
        app.add_systems(
            Update,
            (focus_on_intent, type_when_focused, receive_incoming, render_overlay).chain(),
        );
    }
}

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            ChatOverlay,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(8.0),
                bottom: Val::Px(8.0),
                width: Val::Px(320.0),
                height: Val::Px(180.0),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
        ))
        .with_children(|p| {
            p.spawn((
                ChatHistoryText,
                Text::new("(no messages)"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.75, 0.8, 0.9)),
            ));
            p.spawn((
                ChatInputText,
                Text::new("press T to focus, Esc to blur"),
                TextFont {
                    font_size: FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.5, 0.55, 0.7)),
            ));
        });
}

fn focus_on_intent(mut reader: MessageReader<IntentEvent>, mut cap: ResMut<InputCapture>) {
    for IntentEvent(intent) in reader.read() {
        if *intent == Intent::ChatFocus {
            cap.claim(CHAT_CLAIM);
        }
    }
}

fn type_when_focused(
    cap: Res<InputCapture>,
    mut reader: MessageReader<KeyboardInput>,
    mut state: ResMut<ChatOverlayState>,
    mut writer: MessageWriter<OutgoingChat>,
) {
    let focused = cap.claimants().any(|c| c == CHAT_CLAIM);
    if !focused {
        for _ in reader.read() {}
        return;
    }
    for ev in reader.read() {
        if ev.state != ButtonState::Pressed {
            continue;
        }
        match ev.key_code {
            KeyCode::Backspace => {
                state.buffer.pop();
            }
            KeyCode::Enter => {
                if !state.buffer.is_empty() {
                    let body = std::mem::take(&mut state.buffer);
                    state.history.push_back(ChatEntry {
                        who: "me".to_string(),
                        body: body.clone(),
                    });
                    if state.history.len() > HISTORY_CAP {
                        state.history.pop_front();
                    }
                    writer.write(OutgoingChat(body));
                }
            }
            KeyCode::Escape => {}
            _ => {
                if let Some(text) = &ev.text {
                    for c in text.chars() {
                        if !c.is_control() {
                            state.buffer.push(c);
                        }
                    }
                }
            }
        }
    }
}

fn receive_incoming(
    mut reader: MessageReader<IncomingChat>,
    mut state: ResMut<ChatOverlayState>,
) {
    for IncomingChat(msg) in reader.read() {
        let short_peer: String = msg.peer.chars().take(8).collect();
        state.history.push_back(ChatEntry {
            who: short_peer,
            body: msg.body.clone(),
        });
        if state.history.len() > HISTORY_CAP {
            state.history.pop_front();
        }
    }
}

fn render_overlay(
    state: Res<ChatOverlayState>,
    cap: Res<InputCapture>,
    mut history_texts: Query<&mut Text, (With<ChatHistoryText>, Without<ChatInputText>)>,
    mut input_texts: Query<&mut Text, (With<ChatInputText>, Without<ChatHistoryText>)>,
) {
    if !state.is_changed() && !cap.is_changed() {
        return;
    }
    let history_body = if state.history.is_empty() {
        "(no messages)".to_string()
    } else {
        state
            .history
            .iter()
            .map(|e| format!("{}: {}", e.who, e.body))
            .collect::<Vec<_>>()
            .join("\n")
    };
    for mut t in &mut history_texts {
        if t.0 != history_body {
            t.0 = history_body.clone();
        }
    }
    let focused = cap.claimants().any(|c| c == CHAT_CLAIM);
    let input_body = if focused {
        format!("> {}_", state.buffer)
    } else {
        "press T to focus, Esc to blur".to_string()
    };
    for mut t in &mut input_texts {
        if t.0 != input_body {
            t.0 = input_body.clone();
        }
    }
}
