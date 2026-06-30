#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
unsafe extern "C" {
    #[wasm_bindgen(js_namespace = window, js_name = "__bevyStarterLoadIdentity")]
    fn js_load_identity() -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = window, js_name = "__bevyStarterSaveIdentity")]
    fn js_save_identity(bytes: js_sys::Uint8Array) -> js_sys::Promise;

    #[wasm_bindgen(js_namespace = window, js_name = "__bevyStarterStatus")]
    fn js_status(msg: &str);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async {
        let bytes = load_or_mint_identity().await;
        let kp = laye_me::load(&bytes);
        let msg = match kp {
            Ok(k) => {
                let pk = k.public().try_into_ed25519();
                match pk {
                    Ok(ed) => format!(
                        "bevy-starter loaded — identity {} bytes, pubkey {} bytes",
                        bytes.len(),
                        ed.to_bytes().len()
                    ),
                    Err(e) => format!("bevy-starter loaded — non-Ed25519 public: {e}"),
                }
            }
            Err(e) => format!("bevy-starter load error: {e}"),
        };
        js_status(&msg);
    });
}

#[cfg(target_arch = "wasm32")]
async fn load_or_mint_identity() -> Vec<u8> {
    use wasm_bindgen::JsCast;
    let val = wasm_bindgen_futures::JsFuture::from(js_load_identity())
        .await
        .ok();
    match val {
        Some(v) if !v.is_null() && !v.is_undefined() => {
            if let Ok(arr) = v.dyn_into::<js_sys::Uint8Array>() {
                let mut bytes = vec![0u8; arr.length() as usize];
                arr.copy_to(&mut bytes);
                return bytes;
            }
            mint_and_save().await
        }
        _ => mint_and_save().await,
    }
}

#[cfg(target_arch = "wasm32")]
async fn mint_and_save() -> Vec<u8> {
    let fresh = laye_me::fresh();
    let bytes = laye_me::to_bytes(&fresh).unwrap_or_default();
    let arr = js_sys::Uint8Array::from(bytes.as_slice());
    let _ = wasm_bindgen_futures::JsFuture::from(js_save_identity(arr)).await;
    bytes
}
