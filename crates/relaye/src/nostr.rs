//! Nostr login for the laye identity broker (NIP-46, QR-driven).
//!
//! Broker is the NIP-46 client; user scans a `nostrconnect://` QR
//! with their signer app (Primal, Amber, nsec.app). No browser
//! extension required.
//!
//! Consulted:
//! - <https://nips.nostr.com/46> — NIP-46 spec
//! - <https://buttondown.com/nostrcompass/archive/nostr-compass-4/>
//! - <https://github.com/PrimalHQ/primal-android-app/releases>

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nostrconnect_uri_shape() {
        let pubkey_hex =
            "0000000000000000000000000000000000000000000000000000000000000001";
        let relay = "wss://relay.example.com";
        let secret = "abc123";
        let uri = nostrconnect_uri(pubkey_hex, relay, secret);
        assert_eq!(
            uri,
            "nostrconnect://0000000000000000000000000000000000000000000000000000000000000001?relay=wss%3A%2F%2Frelay.example.com&secret=abc123"
        );
    }
}
