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
}

#[cfg(target_arch = "wasm32")]
fn document() -> Option<web_sys::Document> {
    web_sys::window().and_then(|w| w.document())
}

#[cfg(target_arch = "wasm32")]
fn set_status(msg: &str) {
    if let Some(el) = document().and_then(|d| d.get_element_by_id("status")) {
        el.set_text_content(Some(msg));
    }
}

#[cfg(target_arch = "wasm32")]
fn emit_error(msg: &str) {
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(msg));
    let Some(doc) = document() else { return };
    let Some(container) = doc.get_element_by_id("errors") else {
        return;
    };
    let Ok(line) = doc.create_element("div") else {
        return;
    };
    line.set_class_name("err-line");
    line.set_text_content(Some(msg));
    let _ = container.append_child(&line);
}

#[cfg(target_arch = "wasm32")]
fn show_panic(source: &str, detail: &str) {
    let msg = format!("{source}\n\n{detail}");
    web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(&msg));
    let Some(doc) = document() else { return };
    let Some(el) = doc.get_element_by_id("panic") else {
        return;
    };
    el.set_text_content(Some(&msg));
    let _ = el.class_list().add_1("shown");
}

#[cfg(target_arch = "wasm32")]
pub fn install_wasm_error_layer(
    _app: &mut bevy::app::App,
) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing::field::Visit;
    use bevy::log::tracing::{self, Level, Subscriber};
    use bevy::log::tracing_subscriber::Layer;

    #[derive(Default)]
    struct MessageVisitor {
        message: String,
    }
    impl Visit for MessageVisitor {
        fn record_debug(
            &mut self,
            field: &tracing::field::Field,
            value: &dyn std::fmt::Debug,
        ) {
            if field.name() == "message" {
                self.message = format!("{value:?}");
            }
        }
    }

    struct WasmErrorLayer;
    impl<S: Subscriber> Layer<S> for WasmErrorLayer {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: bevy::log::tracing_subscriber::layer::Context<'_, S>,
        ) {
            let level = *event.metadata().level();
            if level != Level::ERROR && level != Level::WARN {
                return;
            }
            let target = event.metadata().target();
            let mut v = MessageVisitor::default();
            event.record(&mut v);
            let tag = if level == Level::ERROR { "ERROR" } else { "WARN" };
            emit_error(&format!("[{tag}] {target}: {}", v.message));
        }
    }

    Some(Box::new(WasmErrorLayer))
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
        show_panic(
            "rust panic hook",
            &format!("rust panic at {location}: {msg}"),
        );
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
        let (status, identity_bytes) = match load_or_mint_identity().await {
            Ok(bytes) => {
                let s = match laye_me::load(&bytes) {
                    Ok(k) => match k.public().try_into_ed25519() {
                        Ok(ed) => format!(
                            "identity {} bytes, pubkey {} bytes — starting scene",
                            bytes.len(),
                            ed.to_bytes().len()
                        ),
                        Err(e) => format!("non-Ed25519 public: {e}"),
                    },
                    Err(e) => format!("identity load error: {e}"),
                };
                (s, Some(bytes))
            }
            Err(e) => (format!("identity error: {e}"), None),
        };
        set_status(&status);
        scene::build_and_run_app(identity_bytes);
    });

    #[cfg(not(target_arch = "wasm32"))]
    scene::build_and_run_app(None);
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
