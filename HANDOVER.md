# HANDOVER — M2k Nostr login, mid-flight

## Where we are

Branch: `M2k-nostr`. Deployed live to `relaye.sbvh.nl`. End-to-end
does not work: user scans QR in Primal, background-audio keepalive
starts (Primal is in signer mode), broker page silent-hangs, then
`nostr.js` renders "flow deadline exceeded" — a **client-side polling
timeout, not a server-emitted diagnostic**.

We do not know at which step the NIP-46 handshake dies. Any claim
about where would be a guess.

## What's on the branch

- **M2ka–e** — NIP-46 client primitives in `crates/relaye/src/nostr.rs`:
  `nostrconnect://` URI, x-only pubkey, canonical event serialization,
  event id, BIP340 sign/verify, NIP-44 v2 encrypt/decrypt (both
  paulmillr vectors green), NIP-46 request/response codec, kind:24133
  wrap/unwrap, relay `REQ`/`EVENT` builders. Unit-tested against
  spec vectors.
- **M2kf** — `crates/relaye/src/nostr_flow.rs`: `FlowCache`, HTTP
  handlers `handle_start` / `handle_result`, background `run_ws_flow`
  that connects to `wss://relay.damus.io`, subscribes on our ephemeral
  pubkey, expects a signer `connect` request, sends `sign_event`,
  verifies BIP340 sig on the response, emits `laye_me::SignedBinding`
  with `provider: "nostr"`. Routes wired at `/me/sign/nostr/start`
  and `/me/sign/nostr/result`.
- **M2kg** — `broker/` restructured: chooser at `/me/`, provider
  sub-pages at `/me/atproto/`, `/me/mastodon/`, `/me/nostr/`. Nostr
  sub-page renders server-generated QR SVG (`qrcode` crate) and polls
  the result endpoint. `deploy-broker.yml` fires on `**` (was
  `[main]`) so verify-on-branch works. `oauth_atproto` redirect moved
  to `/me/atproto/`; mastodon `REDIRECT_URI` moved to `/me/mastodon/`.

## What blocks progress

`run_ws_flow` violates the sacred-error axiom
(`tsot-roam/ERROR.md`):

- Returns `Result<(), String>` — stringly-typed error envelope at a
  layer boundary. Forbidden.
- Every `.map_err(|e| format!("…"))?` collapses typed failures into
  rot-prone text, then `?`-bubbles to one outer `warn!` on the box.
- No emissions at intermediate states (WS connected, REQ sent, first
  EVENT arrived, decrypt result, connect verified, sign_event sent,
  response arrived, sig verified).
- Result: silent hang on the broker page. Devtools/journalctl are the
  only surfaces, and neither is in front of the user. Also invisible
  to me during dev.

Consequence: I cannot diagnose the actual NIP-46 handshake failure
because I have zero observation of what happens at runtime. Every
theory (spec-interpretation mismatch, Primal reaches a different
relay, wrong secret, wrong kind, etc.) is a guess.

## First move next session

**Independent relay observer** — before touching any code. Spin up a
local WS client that subscribes to `wss://relay.damus.io` for
`{"kinds":[24133],"#p":["<ephemeral_pubkey_hex>"]}`. Trigger a fresh
flow from the broker page. Watch that stream.

- Events arrive → Primal is talking; the bug is in our WS listener
  or NIP-46 handling. Retrofit sacred-error, iterate on real payloads.
- No events → Primal is not publishing to `relay.damus.io`, or is
  publishing to a different relay, or is not scanning
  `nostrconnect://` as a signer at all. Change the relay in the URI or
  prove the flow against a known-good signer (nsec.app) first.

This is the one diagnostic step that gives ground truth without
touching the code.

## Retrofit plan (after diagnostic)

1. Replace `Result<(), String>` in `nostr_flow.rs` with
   `Result<(), laye_error::Error>` at every boundary.
2. Emit typed events through `errpipe::emit(build(...))` at each
   state transition listed above.
3. Add `GET /me/sign/nostr/trace?state=X` returning the emission log
   for the flow (or SSE for live updates).
4. Broker page live-views the trace next to the QR — failures land at
   the surface where they originated.
5. Once observation is real, iterate on the NIP-46 wire until it
   works against a known-good reference signer first (nsec.app),
   then bring Primal in.

## Also unmanaged

**Chooser is a new first-class primitive that shipped as one-off
HTML with three hardcoded tiles.** No `Provider` type, no manifest,
no single render path — adding provider #4 currently means editing
HTML + CSS + creating a sub-folder + updating docs in three places.
Same treatment `CARD.md` and `ERROR.md` get is what this needs. Not
blocking M2k diagnosis; belongs after M2k lands.

## Why we're pausing

Ability to progress is gated on axiom compliance. Without typed
emissions I keep guessing, and guessing has already produced one
non-working shipped flow. Correct move is diagnostic first, retrofit
second, code third — not "another attempt at the wire in the dark."

## Deploy state at pause

- `deploy-relaye`: green on `c98e800`.
- `deploy-broker`: green on `c98e800`.
- `relaye.sbvh.nl/me/` serves the chooser. `/me/sign/nostr/start`
  responds `400` with `peer_pubkey_hex` required — endpoint is live.
- No infra changes needed to resume; no orphan resources.
