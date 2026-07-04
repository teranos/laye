import init, {
  derive_ed25519_pubkey,
  derive_ed25519_sign,
  phrase_to_seed,
  seed_to_phrase,
} from "./laye_derive_wasm.js";

const DB = "laye-me";
const STORE = "seed";
const KEY = "root";
const ALLOWED_ORIGIN_SUFFIX = ".sbvh.nl";
const DEV_ORIGIN_HOSTS = new Set(["localhost", "127.0.0.1"]);

function openDb() {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = (ev) => {
      const db = ev.target.result;
      if (!db.objectStoreNames.contains(STORE)) db.createObjectStore(STORE);
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function loadSeed() {
  const db = await openDb();
  return await new Promise((resolve, reject) => {
    const req = db.transaction(STORE, "readonly").objectStore(STORE).get(KEY);
    req.onsuccess = () => resolve(req.result ?? null);
    req.onerror = () => reject(req.error);
  });
}

async function saveSeed(bytes) {
  const db = await openDb();
  await new Promise((resolve, reject) => {
    const req = db.transaction(STORE, "readwrite").objectStore(STORE).put(bytes, KEY);
    req.onsuccess = () => resolve();
    req.onerror = () => reject(req.error);
  });
}

function originAllowed(origin) {
  if (!origin) return false;
  try {
    const url = new URL(origin);
    if (DEV_ORIGIN_HOSTS.has(url.hostname)) return true;
    return url.hostname === "sbvh.nl" || url.hostname.endsWith(ALLOWED_ORIGIN_SUFFIX);
  } catch {
    return false;
  }
}

const statusEl = document.getElementById("status");
const phraseSection = document.getElementById("phrase-section");
const readySection = document.getElementById("ready-section");
const phraseEl = document.getElementById("phrase");
const importEl = document.getElementById("import");
const importErr = document.getElementById("import-err");

await init();
let seed = await loadSeed();

if (!seed) {
  seed = new Uint8Array(32);
  crypto.getRandomValues(seed);
  await saveSeed(seed);
  phraseEl.textContent = seed_to_phrase(seed);
  phraseSection.classList.remove("hidden");
  statusEl.textContent = "New identity minted. Back up your phrase.";
  document.getElementById("ack").onclick = () => {
    phraseSection.classList.add("hidden");
    readySection.classList.remove("hidden");
    statusEl.textContent = "Identity ready.";
  };
} else {
  readySection.classList.remove("hidden");
  statusEl.textContent = "Identity ready.";
}

document.getElementById("import-btn").onclick = async () => {
  importErr.textContent = "";
  try {
    const bytes = phrase_to_seed(importEl.value.trim());
    seed = new Uint8Array(bytes);
    await saveSeed(seed);
    importEl.value = "";
    statusEl.textContent = "Imported. Identity replaced.";
  } catch (e) {
    importErr.textContent = String(e);
  }
};

window.addEventListener("message", (ev) => {
  if (!originAllowed(ev.origin)) return;
  const { id, action, purpose, msg } = ev.data ?? {};
  let reply;
  try {
    if (action === "pubkey") {
      const pk = derive_ed25519_pubkey(seed, purpose);
      reply = { id, ok: true, pubkey: Array.from(pk) };
    } else if (action === "sign") {
      const sig = derive_ed25519_sign(seed, purpose, new Uint8Array(msg));
      reply = { id, ok: true, sig: Array.from(sig) };
    } else {
      reply = { id, ok: false, err: `unknown action: ${action}` };
    }
  } catch (e) {
    reply = { id, ok: false, err: String(e) };
  }
  ev.source.postMessage(reply, ev.origin);
});
