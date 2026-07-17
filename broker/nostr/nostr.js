const START_ENDPOINT = `${location.origin}/me/sign/nostr/start`;
const RESULT_ENDPOINT = `${location.origin}/me/sign/nostr/result`;

const STORAGE_PEER_PUBKEY = "laye_peer_pubkey_hex";
const STORAGE_MAIN_STATE = "laye_main_state";
const POLL_INTERVAL_MS = 2000;
const POLL_DEADLINE_MS = 5 * 60 * 1000;

document.getElementById("nostr-start").addEventListener("click", startFlow);

async function startFlow() {
  const peerPubkeyHex = sessionStorage.getItem(STORAGE_PEER_PUBKEY);
  if (!peerPubkeyHex) {
    showError("no peer pubkey stashed; open this page via the laye popup");
    return;
  }
  try {
    const r = await fetch(START_ENDPOINT, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        peer_pubkey_hex: peerPubkeyHex,
        main_state: sessionStorage.getItem(STORAGE_MAIN_STATE),
      }),
    });
    const j = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(j.error ?? `HTTP ${r.status}`);
    showQr(j.qr_svg, j.nostrconnect_uri);
    pollResult(j.state);
  } catch (err) {
    showError(err.message);
  }
}

function showQr(qrSvg, nostrconnectUri) {
  document.getElementById("stage-start").hidden = true;
  document.getElementById("stage-qr").hidden = false;
  document.getElementById("qr").innerHTML = qrSvg;
  document.getElementById("nostrconnect-uri").textContent = nostrconnectUri;
}

async function pollResult(state) {
  const deadline = Date.now() + POLL_DEADLINE_MS;
  while (Date.now() < deadline) {
    await sleep(POLL_INTERVAL_MS);
    try {
      const r = await fetch(
        `${RESULT_ENDPOINT}?state=${encodeURIComponent(state)}`,
      );
      if (r.status === 404) continue;
      const j = await r.json().catch(() => ({}));
      if (!r.ok) throw new Error(j.error ?? `HTTP ${r.status}`);
      showResult(j);
      return;
    } catch (err) {
      showError(err.message);
      return;
    }
  }
  showError("flow deadline exceeded; try again");
}

function showResult(signed) {
  document.getElementById("stage-qr").hidden = true;
  document.getElementById("stage-result").hidden = false;
  const link = {
    provider: signed.claim.provider,
    canonical_id: signed.claim.canonical_id,
    handle: signed.claim.handle,
  };
  document.getElementById("result-actor").textContent = JSON.stringify(
    link,
    null,
    2,
  );
  document.getElementById("result-signed").textContent = JSON.stringify(
    signed,
    null,
    2,
  );
  sessionStorage.removeItem(STORAGE_MAIN_STATE);
  if (window.opener && window.opener !== window) {
    window.opener.postMessage(
      { type: "laye/identity/link", link, signed },
      "*",
    );
  }
}

function showError(msg) {
  document.getElementById("nostr-error").textContent = msg;
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
