const iframe = document.getElementById("broker");
const out = document.getElementById("out");
let nextId = 1;
const pending = new Map();

window.addEventListener("message", (ev) => {
  const data = ev.data ?? {};
  const resolver = pending.get(data.id);
  if (resolver) {
    pending.delete(data.id);
    resolver(data);
  }
});

function ask(payload) {
  const id = nextId++;
  return new Promise((resolve) => {
    pending.set(id, resolve);
    iframe.contentWindow.postMessage({ id, ...payload }, "*");
  });
}

document.getElementById("get-pubkey").onclick = async () => {
  out.textContent = "…";
  const purpose = document.getElementById("purpose").value;
  const r = await ask({ action: "pubkey", purpose });
  out.textContent = JSON.stringify(r, null, 2);
};

document.getElementById("sign").onclick = async () => {
  out.textContent = "…";
  const purpose = document.getElementById("purpose").value;
  const msg = Array.from(new TextEncoder().encode("hello"));
  const r = await ask({ action: "sign", purpose, msg });
  out.textContent = JSON.stringify(r, null, 2);
};
