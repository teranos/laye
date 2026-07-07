const REDIRECT_URI = `${location.origin}/me/`;
const CLIENT_NAME = "laye identity broker";
const SCOPES = "read:accounts";

const STORAGE_INSTANCE = "laye_mastodon_instance";
const STORAGE_CLIENT_ID = "laye_mastodon_client_id";
const STORAGE_CLIENT_SECRET = "laye_mastodon_client_secret";

const params = new URLSearchParams(location.search);
const code = params.get("code");
const error = params.get("error");

if (error) {
  showError(`authorization refused: ${error}`);
  clearState();
  history.replaceState({}, "", location.pathname);
} else if (code) {
  const instance = sessionStorage.getItem(STORAGE_INSTANCE);
  const clientId = sessionStorage.getItem(STORAGE_CLIENT_ID);
  const clientSecret = sessionStorage.getItem(STORAGE_CLIENT_SECRET);
  if (!instance || !clientId || !clientSecret) {
    showError("callback arrived without in-progress ceremony state");
  } else {
    handleCallback(code, instance, clientId, clientSecret);
  }
} else {
  setupForm();
}

function setupForm() {
  const form = document.getElementById("mastodon-form");
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const input = document.getElementById("mastodon-instance");
    const inst = normalizeInstance(input.value);
    if (!inst) return;
    try {
      const app = await registerApp(inst);
      sessionStorage.setItem(STORAGE_INSTANCE, inst);
      sessionStorage.setItem(STORAGE_CLIENT_ID, app.client_id);
      sessionStorage.setItem(STORAGE_CLIENT_SECRET, app.client_secret);
      const authUrl = new URL(`https://${inst}/oauth/authorize`);
      authUrl.searchParams.set("client_id", app.client_id);
      authUrl.searchParams.set("redirect_uri", REDIRECT_URI);
      authUrl.searchParams.set("scope", SCOPES);
      authUrl.searchParams.set("response_type", "code");
      location.href = authUrl.toString();
    } catch (err) {
      showError(err.message);
    }
  });
}

function normalizeInstance(raw) {
  let s = raw.trim();
  if (s.startsWith("https://")) s = s.substring(8);
  else if (s.startsWith("http://")) s = s.substring(7);
  while (s.endsWith("/")) s = s.substring(0, s.length - 1);
  return s;
}

async function registerApp(instance) {
  const r = await fetch(`https://${instance}/api/v1/apps`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      client_name: CLIENT_NAME,
      redirect_uris: REDIRECT_URI,
      scopes: SCOPES,
      website: `${location.origin}/me/`,
    }),
  });
  if (!r.ok) throw new Error(`register app on ${instance}: HTTP ${r.status}`);
  return await r.json();
}

async function handleCallback(code, instance, clientId, clientSecret) {
  try {
    const token = await exchangeCode(code, instance, clientId, clientSecret);
    const actor = await fetchActor(token, instance);
    clearState();
    history.replaceState({}, "", location.pathname);
    showResult(actor, instance);
  } catch (err) {
    showError(err.message);
  }
}

async function exchangeCode(code, instance, clientId, clientSecret) {
  const r = await fetch(`https://${instance}/oauth/token`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({
      grant_type: "authorization_code",
      client_id: clientId,
      client_secret: clientSecret,
      redirect_uri: REDIRECT_URI,
      code,
      scope: SCOPES,
    }),
  });
  if (!r.ok) throw new Error(`token exchange: HTTP ${r.status}`);
  const j = await r.json();
  return j.access_token;
}

async function fetchActor(token, instance) {
  const r = await fetch(
    `https://${instance}/api/v1/accounts/verify_credentials`,
    {
      headers: { Authorization: `Bearer ${token}` },
    },
  );
  if (!r.ok) throw new Error(`verify credentials: HTTP ${r.status}`);
  return await r.json();
}

function showResult(actor, instance) {
  document.getElementById("intro").hidden = true;
  document.getElementById("login-mastodon").hidden = true;
  const result = document.getElementById("result");
  result.hidden = false;
  const link = {
    provider: "mastodon",
    canonical_id: actor.url,
    handle: `@${actor.username}@${instance}`,
    display_name: actor.display_name,
    avatar: actor.avatar,
  };
  document.getElementById("result-actor").textContent = JSON.stringify(
    link,
    null,
    2,
  );

  if (window.opener && window.opener !== window) {
    window.opener.postMessage(
      { type: "laye/identity/link", link },
      "*",
    );
  }
}

function showError(msg) {
  const el = document.getElementById("mastodon-error");
  el.textContent = msg;
}

function clearState() {
  sessionStorage.removeItem(STORAGE_INSTANCE);
  sessionStorage.removeItem(STORAGE_CLIENT_ID);
  sessionStorage.removeItem(STORAGE_CLIENT_SECRET);
}
