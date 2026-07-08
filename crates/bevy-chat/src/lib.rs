use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use bevy_input_capture::{InputCapture, Intent, IntentEvent};
use bevy_libp2p::{LayeNet, LibP2PMessage, NetEvent, Topic};
use bevy_me::BindingTable;
use laye_me::SignedChat;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

pub const CHAT_CLAIM: &str = "chat";
pub const HISTORY_CAP: usize = 40;

/// Plaintext wire format matching the pre-M3 rave-chat/v1 shape.
/// Kept for receive-only federation with rave.wasm; laye never
/// publishes in this format.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaintextChat {
    pub peer: String,
    pub body: String,
    pub at_ms: u64,
}

#[derive(Resource)]
pub struct ChatConfig {
    /// Signed topic. laye publishes AND receives here.
    pub topic: Topic,
    /// Legacy plaintext topic. laye receives (read-only) — never
    /// publishes here. `None` disables the plaintext receive path.
    pub legacy_topic: Option<Topic>,
    pub max_body_bytes: usize,
}

#[derive(Message, Debug, Clone)]
pub struct OutgoingChat(pub String);

/// Inbound message from either wire flavor. `Signed` — verified
/// against the author's peer pubkey. `Plaintext` — unauthenticated
/// federation from rave-chat/v1.
#[derive(Message, Debug, Clone)]
pub enum IncomingChat {
    Signed(SignedChat),
    Plaintext(PlaintextChat),
}

pub struct ChatPlugin {
    pub topic: String,
    pub legacy_topic: Option<String>,
    pub max_body_bytes: usize,
}

impl Default for ChatPlugin {
    fn default() -> Self {
        Self {
            topic: "laye-chat/v1".to_string(),
            legacy_topic: Some("rave-chat/v1".to_string()),
            max_body_bytes: 512,
        }
    }
}

impl Plugin for ChatPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ChatConfig {
            topic: Topic(self.topic.clone()),
            legacy_topic: self.legacy_topic.clone().map(Topic),
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
    for OutgoingChat(body) in reader.read() {
        if body.is_empty() {
            continue;
        }
        let trimmed = trim_to_char_boundary(body, cfg.max_body_bytes);
        let Some(bytes) = build_signed_wire(net.keypair(), trimmed, now_ms()) else {
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
    let self_pubkey = self_peer_pubkey(net.keypair());
    let self_peer_id = net.identity().0.clone();
    for msg in reader.read() {
        let NetEvent::Message { topic, bytes, .. } = &msg.0 else {
            continue;
        };
        if topic.0 == cfg.topic.0 {
            let Ok(chat) = serde_json::from_slice::<SignedChat>(bytes) else {
                continue;
            };
            if chat.verify().is_err() {
                continue;
            }
            if let Some(self_pk) = self_pubkey
                && chat.author_peer_pubkey == self_pk
            {
                continue;
            }
            writer.write(IncomingChat::Signed(chat));
        } else if cfg.legacy_topic.as_ref().is_some_and(|t| t.0 == topic.0) {
            let Ok(chat) = serde_json::from_slice::<PlaintextChat>(bytes) else {
                continue;
            };
            if chat.peer == self_peer_id {
                continue;
            }
            writer.write(IncomingChat::Plaintext(chat));
        }
    }
}

/// Build a signed wire payload from a trimmed body and a
/// timestamp. Returns `None` if the local keypair isn't Ed25519,
/// signing fails, or JSON encoding fails.
pub fn build_signed_wire(
    keypair: &bevy_libp2p::Keypair,
    body: String,
    at_ms: u64,
) -> Option<Vec<u8>> {
    let author = self_peer_pubkey(keypair)?;
    let unsigned = SignedChat {
        author_peer_pubkey: author,
        body,
        at_ms,
        signature: Vec::new(),
    };
    let signature = keypair.sign(&unsigned.canonical_bytes()).ok()?;
    let signed = SignedChat {
        signature,
        ..unsigned
    };
    serde_json::to_vec(&signed).ok()
}

fn self_peer_pubkey(keypair: &bevy_libp2p::Keypair) -> Option<[u8; 32]> {
    keypair.public().try_into_ed25519().ok().map(|k| k.to_bytes())
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
    just_focused: bool,
}

#[derive(Component)]
struct ChatOverlay;

#[derive(Component)]
struct ChatHistoryText;

#[derive(Component)]
struct ChatInputText;

#[derive(Default)]
pub struct ChatOverlayPlugin {
    pub initial_history: Vec<String>,
}

impl Plugin for ChatOverlayPlugin {
    fn build(&self, app: &mut App) {
        let mut state = ChatOverlayState::default();
        for line in &self.initial_history {
            state.history.push_back(ChatEntry {
                who: "build".to_string(),
                body: line.clone(),
            });
        }
        app.insert_resource(state);
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

fn focus_on_intent(
    mut reader: MessageReader<IntentEvent>,
    mut cap: ResMut<InputCapture>,
    mut state: ResMut<ChatOverlayState>,
) {
    for IntentEvent(intent) in reader.read() {
        if *intent == Intent::ChatFocus {
            cap.claim(CHAT_CLAIM);
            state.just_focused = true;
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
    if state.just_focused {
        state.just_focused = false;
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
    bindings: Option<Res<BindingTable>>,
) {
    for msg in reader.read() {
        let (who, body) = match msg {
            IncomingChat::Signed(chat) => (
                attribute_author(bindings.as_deref(), &chat.author_peer_pubkey),
                chat.body.clone(),
            ),
            IncomingChat::Plaintext(chat) => {
                (short_peer_display(&chat.peer), chat.body.clone())
            }
        };
        state.history.push_back(ChatEntry { who, body });
        if state.history.len() > HISTORY_CAP {
            state.history.pop_front();
        }
    }
}

/// Short display label for a raw libp2p PeerId string used in
/// plaintext messages. First 8 chars — same as the drawer's
/// `bindings:` log style. Cycle-4 fallback until we resolve
/// PeerId → pubkey → binding for plaintext senders too (which
/// we can't, since plaintext gives no cryptographic proof).
pub fn short_peer_display(peer: &str) -> String {
    peer.chars().take(8).collect()
}

/// Render label for an incoming message's author. If the app has
/// a `BindingTable` and the author's pubkey resolves to a handle,
/// use that (`"@onf@chaos.social"`). Otherwise fall back to the
/// short hex display — the peer is real, just not attributed.
pub fn attribute_author(
    bindings: Option<&BindingTable>,
    author_peer_pubkey: &[u8; 32],
) -> String {
    if let Some(table) = bindings
        && let Some(handle) = table.resolve_handle(author_peer_pubkey)
    {
        return handle.to_string();
    }
    short_author_display(author_peer_pubkey)
}

/// Truncated hex display for an unresolved author pubkey. M3
/// cycle 3 will replace this call site with a `BindingTable`
/// lookup for the real handle.
pub fn short_author_display(pubkey: &[u8; 32]) -> String {
    let mut s = String::with_capacity(8);
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for b in &pubkey[..4] {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn signed_wire_round_trips_a_valid_signature() {
        let kp = laye_me::fresh();
        let bytes = build_signed_wire(&kp, "hello".to_string(), 42)
            .expect("build_signed_wire should succeed for an Ed25519 keypair");
        let parsed: SignedChat = serde_json::from_slice(&bytes).expect("parse");
        parsed.verify().expect("verify");
        assert_eq!(parsed.body, "hello");
        assert_eq!(parsed.at_ms, 42);
    }

    #[test]
    fn short_author_display_is_first_8_hex_chars() {
        let mut pk = [0u8; 32];
        pk[0] = 0x21;
        pk[1] = 0xd7;
        pk[2] = 0x78;
        pk[3] = 0xae;
        assert_eq!(short_author_display(&pk), "21d778ae");
    }

    #[test]
    fn attribute_author_falls_back_to_short_hex_without_binding_table() {
        let mut pk = [0u8; 32];
        pk[0] = 0xca;
        pk[1] = 0xfe;
        pk[2] = 0xba;
        pk[3] = 0xbe;
        assert_eq!(attribute_author(None, &pk), "cafebabe");
    }

    #[test]
    fn plaintext_wire_parses_the_pre_m3_shape() {
        let raw = br#"{"peer":"12D3KooWabc","body":"hi from rave","at_ms":1751888000000}"#;
        let parsed: PlaintextChat =
            serde_json::from_slice(raw).expect("plaintext parses");
        assert_eq!(parsed.peer, "12D3KooWabc");
        assert_eq!(parsed.body, "hi from rave");
        assert_eq!(parsed.at_ms, 1_751_888_000_000);
    }

    #[test]
    fn short_peer_display_takes_first_eight_chars() {
        assert_eq!(short_peer_display("12D3KooWabc1234567"), "12D3KooW");
    }

    #[test]
    fn attribute_author_uses_binding_handle_when_resolved() {
        use laye_me::{BindingClaim, SignedBinding};
        let pk = [0x21; 32];
        let mut table = BindingTable::default();
        table.0.insert(
            pk,
            vec![SignedBinding {
                claim: BindingClaim {
                    peer_pubkey: pk,
                    provider: "mastodon".to_string(),
                    canonical_id: "https://chaos.social/@onf".to_string(),
                    handle: Some("@onf@chaos.social".to_string()),
                    issued_at: 0,
                },
                signature: vec![],
                signer_pubkey: [0u8; 32],
            }],
        );
        assert_eq!(attribute_author(Some(&table), &pk), "@onf@chaos.social");
    }
}
