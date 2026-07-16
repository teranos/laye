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

#[allow(dead_code)] // wired to broker page + relay client in later M2k slices
pub fn nostrconnect_uri(pubkey_hex: &str, relay: &str, secret: &str) -> String {
    let mut out = String::from("nostrconnect://");
    out.push_str(pubkey_hex);
    out.push_str("?relay=");
    percent_encode_into(&mut out, relay);
    out.push_str("&secret=");
    percent_encode_into(&mut out, secret);
    out
}

#[allow(dead_code)] // called by nostrconnect_uri once broker page lands
fn percent_encode_into(out: &mut String, s: &str) {
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{b:02X}"));
            }
        }
    }
}

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
