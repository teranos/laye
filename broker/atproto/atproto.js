const START_ENDPOINT = `${location.origin}/me/oauth/atproto/start`;
const RESULT_ENDPOINT = `${location.origin}/me/sign/atproto/result`;

const STORAGE_PEER_PUBKEY = "laye_peer_pubkey_hex";
const STORAGE_STATE = "laye_atproto_state";
const HISTORY_KEY = "laye_atproto_handle_history";
const HISTORY_CAP = 8;

const params = new URLSearchParams(location.search);
const resultState = params.get("atproto_result");
// Main tab (laye-p2p wasm) passes ?state=<hex> on the initial visit
// so it can poll /me/sign/atproto/result?state=X and survive
// bsky.social's COOP severing window.opener. Stash for the form
// submit that comes after the user picks a handle.
const mainStateFromUrl = params.get("state");
if (mainStateFromUrl) {
  sessionStorage.setItem(STORAGE_STATE, mainStateFromUrl);
}

if (resultState) {
  finalizeAtprotoResult(resultState);
} else {
  setupAtprotoForm();
}

function setupAtprotoForm() {
  const input = document.getElementById("atproto-handle");
  const history = loadHistory();
  if (history.length > 0) {
    input.value = history[0];
    populateHistoryDatalist(history);
  }
  const form = document.getElementById("atproto-form");
  if (!form) return;
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const handle = normalizeHandle(input.value);
    if (!handle) return;
    const peerPubkeyHex = sessionStorage.getItem(STORAGE_PEER_PUBKEY);
    if (!peerPubkeyHex) {
      showError("no peer pubkey stashed; open this page via the laye popup");
      return;
    }
    try {
      const mainState = sessionStorage.getItem(STORAGE_STATE);
      const r = await fetch(START_ENDPOINT, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          peer_pubkey_hex: peerPubkeyHex,
          handle,
          main_state: mainState,
        }),
      });
      const j = await r.json().catch(() => ({}));
      if (!r.ok) {
        throw new Error(j.error ?? `HTTP ${r.status}`);
      }
      remember(handle);
      // Server echoes back state — if we sent main_state it uses ours,
      // otherwise it minted its own. Persist whichever.
      sessionStorage.setItem(STORAGE_STATE, j.state);
      location.href = j.authorize_url;
    } catch (err) {
      showError(err.message);
    }
  });
}

async function finalizeAtprotoResult(state) {
  try {
    const stashed = sessionStorage.getItem(STORAGE_STATE);
    if (stashed && stashed !== state) {
      throw new Error("state mismatch with sessionStorage");
    }
    const r = await fetch(`${RESULT_ENDPOINT}?state=${encodeURIComponent(state)}`);
    const j = await r.json().catch(() => ({}));
    if (!r.ok) {
      throw new Error(j.error ?? `HTTP ${r.status}`);
    }
    sessionStorage.removeItem(STORAGE_STATE);
    history.replaceState({}, "", location.pathname);
    const signed = j;
    const link = {
      provider: signed.claim.provider,
      canonical_id: signed.claim.canonical_id,
      handle: signed.claim.handle,
    };
    showResult(link, signed);
    if (window.opener && window.opener !== window) {
      window.opener.postMessage(
        { type: "laye/identity/link", link, signed },
        "*",
      );
    }
  } catch (err) {
    showError(err.message);
  }
}

function showResult(link, signed) {
  const intro = document.getElementById("intro");
  const mLogin = document.getElementById("login-mastodon");
  const aLogin = document.getElementById("login-atproto");
  if (intro) intro.hidden = true;
  if (mLogin) mLogin.hidden = true;
  if (aLogin) aLogin.hidden = true;
  const result = document.getElementById("result");
  const actor = document.getElementById("result-actor");
  const signedEl = document.getElementById("result-signed");
  if (result) result.hidden = false;
  if (actor) actor.textContent = JSON.stringify(link, null, 2);
  if (signedEl) {
    signedEl.hidden = false;
    signedEl.textContent = JSON.stringify(signed, null, 2);
  }
}

function showError(msg) {
  const el = document.getElementById("atproto-error");
  if (el) el.textContent = msg;
}

function normalizeHandle(raw) {
  let s = raw.trim().toLowerCase();
  if (s.startsWith("@")) s = s.substring(1);
  return s;
}

function loadHistory() {
  try {
    const raw = localStorage.getItem(HISTORY_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((s) => typeof s === "string" && s.length > 0);
  } catch (_) {
    return [];
  }
}

function remember(handle) {
  try {
    const prior = loadHistory().filter((s) => s !== handle);
    const next = [handle, ...prior].slice(0, HISTORY_CAP);
    localStorage.setItem(HISTORY_KEY, JSON.stringify(next));
  } catch (_) {
    // localStorage may be blocked in private mode / quota; pre-fill is
    // a convenience, not a correctness invariant.
  }
}

function populateHistoryDatalist(history) {
  const list = document.getElementById("atproto-history");
  if (!list) return;
  list.innerHTML = "";
  for (const h of history) {
    const opt = document.createElement("option");
    opt.value = h;
    list.appendChild(opt);
  }
}
