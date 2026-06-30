mod scene;

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

    #[wasm_bindgen(js_namespace = window, js_name = "__bevyStarterPanic")]
    fn js_panic(envelope: &str);
}

#[cfg(target_arch = "wasm32")]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown location>".to_string());
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        js_panic(&format!("rust panic at {location}: {msg}"));
    }));
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn trigger_panic_demo() {
    panic!("demo panic from button — errors-sacred end-to-end test");
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn run() {
    #[cfg(target_arch = "wasm32")]
    install_panic_hook();

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async {
        let status = match load_or_mint_identity().await {
            Ok(bytes) => match laye_me::load(&bytes) {
                Ok(k) => match k.public().try_into_ed25519() {
                    Ok(ed) => format!(
                        "identity {} bytes, pubkey {} bytes — starting scene",
                        bytes.len(),
                        ed.to_bytes().len()
                    ),
                    Err(e) => format!("non-Ed25519 public: {e}"),
                },
                Err(e) => format!("identity load error: {e}"),
            },
            Err(e) => format!("identity error: {e}"),
        };
        js_status(&status);
        scene::build_and_run_app();
    });

    #[cfg(not(target_arch = "wasm32"))]
    scene::build_and_run_app();
}

#[cfg(target_arch = "wasm32")]
async fn load_or_mint_identity() -> Result<Vec<u8>, String> {
    use wasm_bindgen::JsCast;
    let val = wasm_bindgen_futures::JsFuture::from(js_load_identity())
        .await
        .map_err(|e| format!("read identity from IndexedDB: {e:?}"))?;
    if !val.is_null()
        && !val.is_undefined()
        && let Ok(arr) = val.dyn_into::<js_sys::Uint8Array>()
    {
        let mut bytes = vec![0u8; arr.length() as usize];
        arr.copy_to(&mut bytes);
        return Ok(bytes);
    }
    mint_and_save().await
}

#[cfg(target_arch = "wasm32")]
async fn mint_and_save() -> Result<Vec<u8>, String> {
    let fresh = laye_me::fresh();
    let bytes = laye_me::to_bytes(&fresh).map_err(|e| format!("encode fresh identity: {e}"))?;
    let arr = js_sys::Uint8Array::from(bytes.as_slice());
    wasm_bindgen_futures::JsFuture::from(js_save_identity(arr))
        .await
        .map_err(|e| format!("save identity to IndexedDB: {e:?}"))?;
    Ok(bytes)
}
