//! NIP-46 nostrconnect flow orchestration for the laye identity broker.
//!
//! One relaye-hosted flow per user click on the broker page. Broker
//! mints an ephemeral secp256k1 keypair, publishes a `nostrconnect://`
//! URI (rendered as a QR by the broker page) and opens a WebSocket to
//! the chosen Nostr relay. Every crypto step below happens here —
//! broker JS is transport UI only.
//!
//! State machine:
//!   Start
//!     -> spawn run_ws_flow(...)
//!     -> return { state, nostrconnect_uri }
//!   run_ws_flow: (over WS)
//!     1. REQ subscription filter on our ephemeral pubkey
//!     2. Await signer's `connect` request (kind:24133 encrypted event)
//!        - verify shared secret from URI matches params[1]
//!        - extract signer's user pubkey from params[0]
//!     3. Publish `sign_event` request — asks the signer to sign a
//!        kind:1 event whose content binds their pubkey to the laye
//!        peer_pubkey ("laye-identity/v1 binding: <hex>")
//!     4. Await signed response; verify BIP340 sig over recomputed id
//!     5. Build SignedBinding{provider:"nostr", canonical_id:<hex>,
//!        handle:None}, sign with relaye's Ed25519 key, insert into
//!        FlowCache under `state`.
//!   result endpoint: broker page + main tab poll GET /me/sign/nostr/result?state=…

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use futures::{SinkExt, StreamExt};
use libp2p::identity::Keypair;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;
use tracing::warn;

use crate::nostr;

const FLOW_TTL: Duration = Duration::from_secs(600);
const WS_TOTAL_TIMEOUT: Duration = Duration::from_secs(300);
const SUBSCRIPTION_ID: &str = "laye-nostr-1";
const REQUEST_ID: &str = "laye-req-1";
const BINDING_KIND: u32 = 1;
const DEFAULT_RELAY: &str = "wss://relay.damus.io";

#[derive(Debug, thiserror::Error)]
pub enum FlowError {
    #[error("bad request json: {0}")]
    BadRequestJson(String),
    #[error("bad peer pubkey")]
    BadPeerPubkey,
    #[error("unknown or expired state")]
    UnknownState,
    #[error("sign: {0}")]
    Sign(String),
    #[error("json: {0}")]
    Json(String),
}

impl FlowError {
    pub fn http_status(&self) -> u16 {
        match self {
            FlowError::BadRequestJson(_) | FlowError::BadPeerPubkey => 400,
            FlowError::UnknownState => 404,
            FlowError::Sign(_) | FlowError::Json(_) => 500,
        }
    }
}

pub struct FlowState {
    pub created_at: Instant,
}

pub struct ResultState {
    pub signed: laye_me::SignedBinding,
    pub created_at: Instant,
}

#[derive(Clone)]
pub struct FlowCache {
    inner: Arc<Mutex<FlowCacheInner>>,
}

struct FlowCacheInner {
    flows: std::collections::HashMap<String, FlowState>,
    results: std::collections::HashMap<String, ResultState>,
}

impl FlowCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FlowCacheInner {
                flows: std::collections::HashMap::new(),
                results: std::collections::HashMap::new(),
            })),
        }
    }

    pub fn insert_flow(&self, state: String, flow: FlowState) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        gc(&mut inner);
        inner.flows.insert(state, flow);
    }

    pub fn insert_result(&self, state: String, signed: laye_me::SignedBinding) {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        gc(&mut inner);
        inner.results.insert(
            state,
            ResultState {
                signed,
                created_at: Instant::now(),
            },
        );
    }

    /// Non-draining: both the broker page and the main tab poll the
    /// same result endpoint; TTL cleans up.
    pub fn get_result(&self, state: &str) -> Option<laye_me::SignedBinding> {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        gc(&mut inner);
        inner.results.get(state).map(|r| r.signed.clone())
    }
}

impl Default for FlowCache {
    fn default() -> Self {
        Self::new()
    }
}

fn gc(inner: &mut FlowCacheInner) {
    let now = Instant::now();
    inner
        .flows
        .retain(|_, f| now.duration_since(f.created_at) < FLOW_TTL);
    inner
        .results
        .retain(|_, r| now.duration_since(r.created_at) < FLOW_TTL);
}

// ============================================================================
// HTTP handlers
// ============================================================================

#[derive(Deserialize)]
pub struct StartRequest {
    pub peer_pubkey_hex: String,
    #[serde(default)]
    pub main_state: Option<String>,
    #[serde(default)]
    pub relay: Option<String>,
}

#[derive(Serialize)]
pub struct StartResponse {
    pub state: String,
    pub nostrconnect_uri: String,
}

pub async fn handle_start(
    body: &[u8],
    cache: &FlowCache,
    relay_signing_key: &Keypair,
) -> Result<Vec<u8>, FlowError> {
    let req: StartRequest = serde_json::from_slice(body)
        .map_err(|e| FlowError::BadRequestJson(format!("StartRequest: {e}")))?;
    let peer_pubkey_hex = normalize_peer_pubkey(&req.peer_pubkey_hex)?;
    let relay = req.relay.unwrap_or_else(|| DEFAULT_RELAY.to_string());

    let mut ephemeral_secret = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut ephemeral_secret);
    let ephemeral_pubkey_hex = nostr::x_only_pubkey_hex(&ephemeral_secret)
        .map_err(|e| FlowError::Sign(format!("ephemeral: {e}")))?;
    let mut secret_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut secret_bytes);
    let connect_secret = hex::encode(secret_bytes);
    let state = req.main_state.clone().unwrap_or_else(random_state);

    let nostrconnect_uri = nostr::nostrconnect_uri(&ephemeral_pubkey_hex, &relay, &connect_secret);

    cache.insert_flow(
        state.clone(),
        FlowState {
            created_at: Instant::now(),
        },
    );

    // Spawn WS orchestration. If it errors, the flow simply times out
    // in FlowCache and the polling clients see UnknownState after TTL.
    let cache_bg = cache.clone();
    let key_bg = relay_signing_key.clone();
    let state_bg = state.clone();
    tokio::spawn(async move {
        if let Err(e) = run_ws_flow(
            relay,
            ephemeral_secret,
            ephemeral_pubkey_hex,
            connect_secret,
            peer_pubkey_hex,
            state_bg,
            cache_bg,
            key_bg,
        )
        .await
        {
            warn!(error = %e, "nostr flow ws task failed");
        }
    });

    serde_json::to_vec(&StartResponse {
        state,
        nostrconnect_uri,
    })
    .map_err(|e| FlowError::Json(e.to_string()))
}

pub async fn handle_result(
    query: &std::collections::HashMap<String, String>,
    cache: &FlowCache,
) -> Result<Vec<u8>, FlowError> {
    let state = query
        .get("state")
        .cloned()
        .ok_or_else(|| FlowError::BadRequestJson("missing ?state".into()))?;
    let signed = cache.get_result(&state).ok_or(FlowError::UnknownState)?;
    serde_json::to_vec(&signed).map_err(|e| FlowError::Json(e.to_string()))
}

// ============================================================================
// WS orchestration (spawned per flow)
// ============================================================================

#[allow(clippy::too_many_arguments)]
async fn run_ws_flow(
    relay_url: String,
    ephemeral_secret: [u8; 32],
    ephemeral_pubkey_hex: String,
    connect_secret: String,
    peer_pubkey_hex: String,
    state: String,
    cache: FlowCache,
    relay_signing_key: Keypair,
) -> Result<(), String> {
    let (mut ws, _) = tokio_tungstenite::connect_async(&relay_url)
        .await
        .map_err(|e| format!("connect_async: {e}"))?;

    ws.send(Message::Text(nostr::relay_req_message(
        SUBSCRIPTION_ID,
        &ephemeral_pubkey_hex,
    )))
    .await
    .map_err(|e| format!("send REQ: {e}"))?;

    let mut signer_pubkey: Option<String> = None;
    let deadline = Instant::now() + WS_TOTAL_TIMEOUT;

    while let Some(msg) = tokio::time::timeout(WS_TOTAL_TIMEOUT, ws.next())
        .await
        .map_err(|_| "ws stream timed out".to_string())?
    {
        if Instant::now() > deadline {
            return Err("total flow deadline exceeded".into());
        }
        let msg = msg.map_err(|e| format!("ws recv: {e}"))?;
        let text = match msg {
            Message::Text(t) => t,
            Message::Ping(p) => {
                let _ = ws.send(Message::Pong(p)).await;
                continue;
            }
            Message::Close(_) => return Err("relay closed connection".into()),
            _ => continue,
        };

        let event = match parse_relay_event(&text) {
            Some(e) => e,
            None => continue,
        };

        let sender_pubkey = event
            .get("pubkey")
            .and_then(|v| v.as_str())
            .ok_or("event missing pubkey")?
            .to_string();

        let event_json_str = event.to_string();

        match signer_pubkey.as_ref() {
            None => {
                // Expecting a connect request.
                let plaintext = nostr::nip46_unwrap(
                    &ephemeral_secret,
                    &sender_pubkey,
                    &event_json_str,
                )
                .ok_or("connect: nip46 unwrap failed")?;
                let text = std::str::from_utf8(&plaintext)
                    .map_err(|e| format!("connect utf8: {e}"))?;
                match extract_connect(text, &connect_secret) {
                    Some(signer_pk) if signer_pk == sender_pubkey => {
                        signer_pubkey = Some(signer_pk.clone());
                        send_sign_event_request(
                            &mut ws,
                            &ephemeral_secret,
                            &ephemeral_pubkey_hex,
                            &signer_pk,
                            &peer_pubkey_hex,
                        )
                        .await?;
                    }
                    Some(_) => return Err("connect: pubkey mismatch".into()),
                    None => return Err("connect: secret mismatch or malformed".into()),
                }
            }
            Some(expected) if expected == &sender_pubkey => {
                // Expecting sign_event response.
                let plaintext = nostr::nip46_unwrap(
                    &ephemeral_secret,
                    &sender_pubkey,
                    &event_json_str,
                )
                .ok_or("response: nip46 unwrap failed")?;
                let text = std::str::from_utf8(&plaintext)
                    .map_err(|e| format!("response utf8: {e}"))?;
                let signed_ev = verify_sign_event_response(text, &sender_pubkey)?;
                emit_signed_binding(
                    &cache,
                    &state,
                    &peer_pubkey_hex,
                    &sender_pubkey,
                    signed_ev.created_at,
                    &relay_signing_key,
                )?;
                return Ok(());
            }
            Some(_) => continue, // event from someone we're not talking to
        }
    }
    Err("ws stream ended without result".into())
}

// ============================================================================
// Testable primitives that run_ws_flow composes.
// ============================================================================

fn parse_relay_event(raw: &str) -> Option<serde_json::Value> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let arr = v.as_array()?;
    if arr.len() < 3 || arr[0].as_str()? != "EVENT" {
        return None;
    }
    Some(arr[2].clone())
}

/// If the plaintext is a valid NIP-46 `connect` request whose secret
/// matches `expected_secret`, returns the signer's user pubkey (from
/// params[0]). None otherwise.
fn extract_connect(plaintext: &str, expected_secret: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(plaintext).ok()?;
    let method = v.get("method")?.as_str()?;
    if method != "connect" {
        return None;
    }
    let params = v.get("params")?.as_array()?;
    let signer_pubkey = params.first()?.as_str()?.to_string();
    let secret = params.get(1)?.as_str()?;
    if secret != expected_secret {
        return None;
    }
    Some(signer_pubkey)
}

async fn send_sign_event_request(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    ephemeral_secret: &[u8; 32],
    ephemeral_pubkey_hex: &str,
    signer_pubkey_hex: &str,
    peer_pubkey_hex: &str,
) -> Result<(), String> {
    let created_at = now_unix_secs();
    let event_to_sign = build_binding_event_json(signer_pubkey_hex, peer_pubkey_hex, created_at);
    let request = nostr::nip46_request_json(REQUEST_ID, "sign_event", &[&event_to_sign]);

    let mut nonce = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut nonce);
    let mut aux = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut aux);

    let wrapped = nostr::nip46_wrap(
        ephemeral_secret,
        ephemeral_pubkey_hex,
        signer_pubkey_hex,
        request.as_bytes(),
        &nonce,
        created_at,
        &aux,
    )
    .ok_or("nip46 wrap failed")?;

    let event_msg = nostr::relay_event_message(&wrapped);
    ws.send(Message::Text(event_msg))
        .await
        .map_err(|e| format!("send EVENT: {e}"))?;
    Ok(())
}

fn build_binding_event_json(
    signer_pubkey_hex: &str,
    peer_pubkey_hex: &str,
    created_at: u64,
) -> String {
    // Unsigned event skeleton — signer fills id + sig.
    format!(
        r#"{{"kind":{BINDING_KIND},"content":"laye-identity/v1 binding: {peer_pubkey_hex}","tags":[],"created_at":{created_at},"pubkey":"{signer_pubkey_hex}"}}"#
    )
}

struct VerifiedBinding {
    created_at: u64,
}

fn verify_sign_event_response(
    plaintext: &str,
    expected_signer_pubkey: &str,
) -> Result<VerifiedBinding, String> {
    let resp = nostr::parse_nip46_response(plaintext).ok_or("response: parse failed")?;
    let result = resp.result.ok_or_else(|| {
        resp.error
            .unwrap_or_else(|| "response: missing result".into())
    })?;
    let ev: serde_json::Value = serde_json::from_str(&result)
        .map_err(|e| format!("response: signed-event json: {e}"))?;

    let pubkey = ev
        .get("pubkey")
        .and_then(|v| v.as_str())
        .ok_or("response: pubkey missing")?;
    if pubkey != expected_signer_pubkey {
        return Err("response: pubkey mismatch".into());
    }
    let sig_hex = ev
        .get("sig")
        .and_then(|v| v.as_str())
        .ok_or("response: sig missing")?;
    let created_at = ev
        .get("created_at")
        .and_then(|v| v.as_u64())
        .ok_or("response: created_at missing")?;
    let kind = ev
        .get("kind")
        .and_then(|v| v.as_u64())
        .ok_or("response: kind missing")? as u32;
    let content = ev
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or("response: content missing")?;
    let tags_json = serde_json::to_string(ev.get("tags").ok_or("response: tags missing")?)
        .map_err(|e| format!("response: tags json: {e}"))?;

    let recomputed = nostr::event_id(pubkey, created_at, kind, &tags_json, content);
    if !nostr::verify_schnorr(&recomputed, sig_hex, pubkey) {
        return Err("response: schnorr verify failed".into());
    }
    Ok(VerifiedBinding { created_at })
}

fn emit_signed_binding(
    cache: &FlowCache,
    state: &str,
    peer_pubkey_hex: &str,
    signer_pubkey_hex: &str,
    issued_at: u64,
    relay_signing_key: &Keypair,
) -> Result<(), String> {
    let peer_pubkey = decode_peer_pubkey(peer_pubkey_hex).map_err(|e| e.to_string())?;
    let claim = laye_me::BindingClaim {
        peer_pubkey,
        provider: "nostr".to_string(),
        canonical_id: signer_pubkey_hex.to_string(),
        handle: None,
        issued_at,
    };
    let canonical = claim.canonical_bytes();
    let signature = relay_signing_key
        .sign(&canonical)
        .map_err(|e| format!("sign: {e}"))?;
    let signer_pubkey = relay_signing_key
        .public()
        .try_into_ed25519()
        .map_err(|e| format!("relay pubkey not Ed25519: {e}"))?
        .to_bytes();
    cache.insert_result(
        state.to_string(),
        laye_me::SignedBinding {
            claim,
            signature,
            signer_pubkey,
        },
    );
    Ok(())
}

fn normalize_peer_pubkey(hex_str: &str) -> Result<String, FlowError> {
    let bytes = hex::decode(hex_str).map_err(|_| FlowError::BadPeerPubkey)?;
    if bytes.len() != 32 {
        return Err(FlowError::BadPeerPubkey);
    }
    Ok(hex::encode(bytes))
}

fn decode_peer_pubkey(hex_str: &str) -> Result<[u8; 32], FlowError> {
    let bytes = hex::decode(hex_str).map_err(|_| FlowError::BadPeerPubkey)?;
    if bytes.len() != 32 {
        return Err(FlowError::BadPeerPubkey);
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn random_state() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_relay_event_accepts_valid_frame() {
        let raw = r#"["EVENT","sub-1",{"pubkey":"a","kind":24133}]"#;
        let ev = parse_relay_event(raw).unwrap();
        assert_eq!(ev["pubkey"], "a");
    }

    #[test]
    fn parse_relay_event_rejects_non_event() {
        assert!(parse_relay_event(r#"["OK","evtid",true,""]"#).is_none());
        assert!(parse_relay_event(r#"["EOSE","sub-1"]"#).is_none());
        assert!(parse_relay_event(r#"not json"#).is_none());
    }

    #[test]
    fn extract_connect_accepts_matching_secret() {
        let plaintext =
            r#"{"id":"1","method":"connect","params":["signer-pk","the-secret"]}"#;
        assert_eq!(
            extract_connect(plaintext, "the-secret").as_deref(),
            Some("signer-pk")
        );
    }

    #[test]
    fn extract_connect_rejects_wrong_secret() {
        let plaintext =
            r#"{"id":"1","method":"connect","params":["signer-pk","evil"]}"#;
        assert!(extract_connect(plaintext, "expected").is_none());
    }

    #[test]
    fn extract_connect_rejects_non_connect_method() {
        let plaintext =
            r#"{"id":"1","method":"sign_event","params":["signer-pk","the-secret"]}"#;
        assert!(extract_connect(plaintext, "the-secret").is_none());
    }

    /// End-to-end round-trip: a synthetic "signer" produces a signed
    /// binding event exactly as a Primal/Amber signer would, then
    /// verify_sign_event_response accepts it.
    #[test]
    fn verify_sign_event_response_accepts_signer_produced_binding() {
        use crate::nostr::{event_id, sign_schnorr, x_only_pubkey_hex};

        let mut signer_sec = [0u8; 32];
        signer_sec[31] = 42;
        let signer_pk = x_only_pubkey_hex(&signer_sec).unwrap();

        let peer_pubkey_hex = "deadbeef".repeat(8);
        let created_at = 1_700_000_000;
        let content = format!("laye-identity/v1 binding: {peer_pubkey_hex}");
        let tags_json = "[]";

        let id = event_id(&signer_pk, created_at, BINDING_KIND, tags_json, &content);
        let sig = sign_schnorr(&signer_sec, &id, &[0u8; 32]).unwrap();

        // Signer wire-shape response: result = signed-event JSON.
        let signed_event = format!(
            r#"{{"id":"{}","pubkey":"{signer_pk}","created_at":{created_at},"kind":{BINDING_KIND},"tags":[],"content":"{content}","sig":"{sig}"}}"#,
            hex::encode(id)
        );
        // NIP-46 wrap-shape response: {"id","result":<signed-event as string>}
        let response = format!(
            r#"{{"id":"laye-req-1","result":{}}}"#,
            serde_json::to_string(&signed_event).unwrap()
        );

        let verified = verify_sign_event_response(&response, &signer_pk).unwrap();
        assert_eq!(verified.created_at, created_at);
    }

    #[test]
    fn verify_sign_event_response_rejects_pubkey_mismatch() {
        use crate::nostr::{event_id, sign_schnorr, x_only_pubkey_hex};

        let mut signer_sec = [0u8; 32];
        signer_sec[31] = 42;
        let signer_pk = x_only_pubkey_hex(&signer_sec).unwrap();

        let created_at = 1_700_000_000;
        let content = "laye-identity/v1 binding: deadbeef";
        let id = event_id(&signer_pk, created_at, BINDING_KIND, "[]", content);
        let sig = sign_schnorr(&signer_sec, &id, &[0u8; 32]).unwrap();

        let signed_event = format!(
            r#"{{"id":"{}","pubkey":"{signer_pk}","created_at":{created_at},"kind":{BINDING_KIND},"tags":[],"content":"{content}","sig":"{sig}"}}"#,
            hex::encode(id)
        );
        let response = format!(
            r#"{{"id":"laye-req-1","result":{}}}"#,
            serde_json::to_string(&signed_event).unwrap()
        );

        // Verify with a different expected pubkey.
        assert!(verify_sign_event_response(&response, "not-the-signer").is_err());
    }

    #[test]
    fn flow_cache_result_is_non_draining() {
        let cache = FlowCache::new();
        let signed = fake_signed_binding();
        cache.insert_result("state-1".into(), signed.clone());
        assert!(cache.get_result("state-1").is_some());
        // Second read returns the same thing.
        assert!(cache.get_result("state-1").is_some());
    }

    fn fake_signed_binding() -> laye_me::SignedBinding {
        laye_me::SignedBinding {
            claim: laye_me::BindingClaim {
                peer_pubkey: [0u8; 32],
                provider: "nostr".into(),
                canonical_id: "signer".into(),
                handle: None,
                issued_at: 1,
            },
            signature: vec![0u8; 64],
            signer_pubkey: [0u8; 32],
        }
    }
}
