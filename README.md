# laye

Browser social plugin. Loads beside any host wasm in the same tab.
Owns identity (Ed25519 + IndexedDB), signed chat, login (Mastodon
today), and libp2p transport. Hosts reach it through `window.laye`.

## Integrate

```html
<script type="module">
  const laye = await import("./laye_p2p.js");
  await laye.default();
  await laye.init(JSON.stringify({
    bootstrap_addrs: ["/dns4/relaye.sbvh.nl/tcp/443/wss/p2p/12D3KooWC6UBnnmhhv3BAfYKyW1bFBD4GtC5waiEgQWJCb7Hbqaf"],
    topics: [],
    identify_protocol: "/your-app/1.0.0",
  }));
  window.laye = laye;
</script>
```

Then load your wasm.

See `bevy-starter/web/index.html` for a working host.
