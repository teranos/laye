//! JS extern bindings to laye-p2p.wasm (the sibling wasm module).
//!
//! bevy-starter loads laye-p2p first in index.html, which installs
//! `window.laye` = the ES module namespace. Positions flow through the
//! four-verb opaque API on rave-positions/v1. Identity, chat, login,
//! errors live entirely inside laye-p2p and its DOM overlays.
//!
//! Fire-and-forget contract: none of these functions throw. Any laye-p2p
//! failure surfaces as a typed Error in the sacred red overlay per
//! ERROR.md's visual contract — bevy-starter neither catches nor
//! renders errors from the seam.

use wasm_bindgen::prelude::*;

#[wasm_bindgen(js_namespace = ["window", "laye"])]
extern "C" {
    #[wasm_bindgen(js_name = self_peer_id)]
    pub fn self_peer_id() -> String;

    #[wasm_bindgen(js_name = subscribe_opaque)]
    pub fn subscribe_opaque(topic: &str);

    #[wasm_bindgen(js_name = publish)]
    pub fn publish(topic: &str, bytes: Vec<u8>);

    #[wasm_bindgen(js_name = pending_bytes)]
    pub fn pending_bytes(topic: &str) -> u32;

    #[wasm_bindgen(js_name = recv_bytes)]
    pub fn recv_bytes(topic: &str) -> Vec<u8>;

    #[wasm_bindgen(js_name = laye_is_focused)]
    pub fn laye_is_focused() -> bool;

    #[wasm_bindgen(js_name = emit_error)]
    pub fn emit_error(error_json: &str);
}

/// Framed rx buffer split. Bytes come in as concatenated
/// [u32 LE len][bytes]... frames — the wire format laye-p2p writes and
/// game.wasm expects.
pub fn split_frames(buf: &[u8]) -> Vec<Vec<u8>> {
    let mut frames = Vec::new();
    let mut off = 0usize;
    while off + 4 <= buf.len() {
        let len = u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
            as usize;
        off += 4;
        if off + len > buf.len() {
            break;
        }
        frames.push(buf[off..off + len].to_vec());
        off += len;
    }
    frames
}
