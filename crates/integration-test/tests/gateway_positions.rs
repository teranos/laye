//! End-to-end for the R16 WS gateway. Spawns the real relaye
//! binary on loopback, opens two WS clients on
//! /ws/rave-positions/v1, and asserts that a binary frame from
//! one shows up byte-for-byte at the other. Acceptance bar per
//! `crates/relaye/docs/gateway.md`.
//!
//! Runs `cargo build --bin relaye` as its first step so the
//! binary is guaranteed present at `target/<profile>/relaye`.

use std::process::Stdio;
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use tokio::process::Command;
use tokio_tungstenite::tungstenite::Message;

const TOPIC_PATH: &str = "/ws/rave-positions/v1";
const DEPLOYED_URL: &str = "wss://relaye.sbvh.nl/ws/rave-positions/v1";
const RELAYE_STARTUP_TIMEOUT: Duration = Duration::from_secs(15);
const MSG_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

fn pick_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap_or_else(|e| panic!("bind 0: {e}"));
    listener
        .local_addr()
        .unwrap_or_else(|e| panic!("local_addr: {e}"))
        .port()
}

fn workspace_target_dir() -> std::path::PathBuf {
    std::env::var("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target")
        })
}

fn build_relaye() -> std::path::PathBuf {
    let build = std::process::Command::new(env!("CARGO"))
        .args(["build", "--package", "relaye", "--bin", "relaye"])
        .status()
        .unwrap_or_else(|e| panic!("cargo build spawn: {e}"));
    assert!(build.success(), "cargo build --bin relaye failed");

    let bin = workspace_target_dir().join("debug").join("relaye");
    assert!(bin.exists(), "relaye binary missing at {bin:?}");
    bin
}

async fn wait_for_tcp(port: u16) {
    let deadline = std::time::Instant::now() + RELAYE_STARTUP_TIMEOUT;
    loop {
        if tokio::net::TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
            return;
        }
        if std::time::Instant::now() >= deadline {
            panic!("relaye didn't come up on 127.0.0.1:{port} within {RELAYE_STARTUP_TIMEOUT:?}");
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ws_binary_frame_from_one_client_reaches_the_other_client() {
    let bin = build_relaye();
    let public = pick_free_port();
    let internal = pick_free_port();

    let mut relaye = Command::new(&bin)
        .env("RELAYE_LISTEN_HOST", "127.0.0.1")
        .env("RELAYE_LISTEN_PORT", public.to_string())
        .env("RELAYE_INTERNAL_PORT", internal.to_string())
        .env("RELAYE_TOPICS", "rave-positions/v1")
        .env("RELAYE_METRICS_INTERVAL_SECS", "3600") // don't spam
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap_or_else(|e| panic!("spawn relaye: {e}"));

    wait_for_tcp(public).await;
    // Give gossipsub a moment to subscribe to the topic.
    tokio::time::sleep(Duration::from_millis(500)).await;

    let url = format!("ws://127.0.0.1:{public}{TOPIC_PATH}");
    let (ws_a, _) = tokio_tungstenite::connect_async(&url)
        .await
        .unwrap_or_else(|e| panic!("client A connect: {e}"));
    let (ws_b, _) = tokio_tungstenite::connect_async(&url)
        .await
        .unwrap_or_else(|e| panic!("client B connect: {e}"));

    let (mut a_sink, _a_stream) = ws_a.split();
    let (_b_sink, mut b_stream) = ws_b.split();

    // Small settle so both subscriptions are registered before the send.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let payload: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0x01, 0x02, 0x03, 0x04];
    a_sink
        .send(Message::Binary(payload.clone()))
        .await
        .unwrap_or_else(|e| panic!("A send: {e}"));

    let received = tokio::time::timeout(MSG_WAIT_TIMEOUT, async {
        loop {
            match b_stream.next().await {
                Some(Ok(Message::Binary(b))) => return b,
                Some(Ok(Message::Close(_))) | None => panic!("B closed before receiving"),
                Some(Ok(_)) => continue, // skip ping/pong/text/frame
                Some(Err(e)) => panic!("B error: {e}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("B did not receive within {MSG_WAIT_TIMEOUT:?}"));

    assert_eq!(received, payload);

    relaye.kill().await.unwrap_or_else(|e| panic!("kill relaye: {e}"));
}

/// Same round-trip check but hits the deployed WSS endpoint
/// through CloudFront + lightsail. Ignored by default so it
/// doesn't run in offline CI; invoke with
/// `cargo test -p integration-test -- --ignored --nocapture
/// ws_round_trip_via_deployed_cloudfront`.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore]
async fn ws_round_trip_via_deployed_cloudfront() {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let (ws_a, _) = tokio_tungstenite::connect_async(DEPLOYED_URL)
        .await
        .unwrap_or_else(|e| panic!("client A connect (deployed): {e}"));
    let (ws_b, _) = tokio_tungstenite::connect_async(DEPLOYED_URL)
        .await
        .unwrap_or_else(|e| panic!("client B connect (deployed): {e}"));

    let (mut a_sink, _a_stream) = ws_a.split();
    let (_b_sink, mut b_stream) = ws_b.split();

    tokio::time::sleep(Duration::from_millis(300)).await;

    let payload: Vec<u8> = vec![0xca, 0xfe, 0xba, 0xbe];
    a_sink
        .send(Message::Binary(payload.clone()))
        .await
        .unwrap_or_else(|e| panic!("A send (deployed): {e}"));

    let received = tokio::time::timeout(MSG_WAIT_TIMEOUT, async {
        loop {
            match b_stream.next().await {
                Some(Ok(Message::Binary(b))) => return b,
                Some(Ok(Message::Close(_))) | None => panic!("B closed"),
                Some(Ok(_)) => continue,
                Some(Err(e)) => panic!("B error: {e}"),
            }
        }
    })
    .await
    .unwrap_or_else(|_| panic!("B did not receive within {MSG_WAIT_TIMEOUT:?}"));

    assert_eq!(received, payload);
}
