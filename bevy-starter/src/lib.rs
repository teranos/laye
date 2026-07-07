mod scene;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
const IDB_NAME: &str = "bevy-starter";
#[cfg(target_arch = "wasm32")]
const IDB_STORE: &str = "identity";
#[cfg(target_arch = "wasm32")]
const IDB_KEY: &str = "self";

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
    let db = idb_open().await?;
    let val = idb_get(&db, IDB_KEY).await?;
    if !val.is_null()
        && !val.is_undefined()
        && let Ok(arr) = val.dyn_into::<js_sys::Uint8Array>()
    {
        let mut bytes = vec![0u8; arr.length() as usize];
        arr.copy_to(&mut bytes);
        return Ok(bytes);
    }
    mint_and_save(&db).await
}

#[cfg(target_arch = "wasm32")]
async fn mint_and_save(db: &web_sys::IdbDatabase) -> Result<Vec<u8>, String> {
    let fresh = laye_me::fresh();
    let bytes = laye_me::to_bytes(&fresh).map_err(|e| format!("encode fresh identity: {e}"))?;
    let arr = js_sys::Uint8Array::from(bytes.as_slice());
    idb_put(db, IDB_KEY, &arr.into()).await?;
    Ok(bytes)
}

#[cfg(target_arch = "wasm32")]
async fn idb_open() -> Result<web_sys::IdbDatabase, String> {
    use wasm_bindgen::JsCast;
    let factory = web_sys::window()
        .ok_or_else(|| "no window".to_string())?
        .indexed_db()
        .map_err(|e| format!("indexed_db(): {e:?}"))?
        .ok_or_else(|| "indexedDB unavailable".to_string())?;
    let req = factory
        .open_with_u32(IDB_NAME, 1)
        .map_err(|e| format!("open(): {e:?}"))?;

    let upgrade_req = req.clone();
    let onupgrade = wasm_bindgen::closure::Closure::<
        dyn FnMut(web_sys::IdbVersionChangeEvent),
    >::new(move |_ev: web_sys::IdbVersionChangeEvent| {
        let Ok(val) = upgrade_req.result() else { return };
        let Ok(db) = val.dyn_into::<web_sys::IdbDatabase>() else {
            return;
        };
        let names = db.object_store_names();
        let mut has_store = false;
        for i in 0..names.length() {
            if names.item(i).as_deref() == Some(IDB_STORE) {
                has_store = true;
                break;
            }
        }
        if !has_store {
            let _ = db.create_object_store(IDB_STORE);
        }
    });
    req.set_onupgradeneeded(Some(onupgrade.as_ref().unchecked_ref()));
    onupgrade.forget();

    let val = idb_request_promise(req.unchecked_ref::<web_sys::IdbRequest>()).await?;
    val.dyn_into::<web_sys::IdbDatabase>()
        .map_err(|_| "IdbOpenDbRequest result was not an IdbDatabase".to_string())
}

#[cfg(target_arch = "wasm32")]
async fn idb_get(
    db: &web_sys::IdbDatabase,
    key: &str,
) -> Result<wasm_bindgen::JsValue, String> {
    let tx = db
        .transaction_with_str(IDB_STORE)
        .map_err(|e| format!("transaction(readonly): {e:?}"))?;
    let store = tx
        .object_store(IDB_STORE)
        .map_err(|e| format!("object_store: {e:?}"))?;
    let req = store
        .get(&wasm_bindgen::JsValue::from_str(key))
        .map_err(|e| format!("get: {e:?}"))?;
    idb_request_promise(&req).await
}

#[cfg(target_arch = "wasm32")]
async fn idb_put(
    db: &web_sys::IdbDatabase,
    key: &str,
    value: &wasm_bindgen::JsValue,
) -> Result<(), String> {
    let tx = db
        .transaction_with_str_and_mode(IDB_STORE, web_sys::IdbTransactionMode::Readwrite)
        .map_err(|e| format!("transaction(readwrite): {e:?}"))?;
    let store = tx
        .object_store(IDB_STORE)
        .map_err(|e| format!("object_store: {e:?}"))?;
    let req = store
        .put_with_key(value, &wasm_bindgen::JsValue::from_str(key))
        .map_err(|e| format!("put_with_key: {e:?}"))?;
    idb_request_promise(&req).await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn idb_request_promise(
    req: &web_sys::IdbRequest,
) -> Result<wasm_bindgen::JsValue, String> {
    use wasm_bindgen::JsCast;
    let success_req = req.clone();
    let error_req = req.clone();
    let promise = js_sys::Promise::new(&mut move |resolve, reject| {
        let success_req_inner = success_req.clone();
        let resolve_clone = resolve.clone();
        let onsuccess = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_ev: web_sys::Event| match success_req_inner.result() {
                Ok(val) => {
                    let _ = resolve_clone.call1(&wasm_bindgen::JsValue::UNDEFINED, &val);
                }
                Err(e) => {
                    let _ = resolve_clone.call1(&wasm_bindgen::JsValue::UNDEFINED, &e);
                }
            },
        );
        success_req.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        onsuccess.forget();

        let error_req_inner = error_req.clone();
        let onerror = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::Event)>::new(
            move |_ev: web_sys::Event| {
                let msg = error_req_inner
                    .error()
                    .ok()
                    .flatten()
                    .map(|e| e.message())
                    .unwrap_or_else(|| "unknown IDB error".to_string());
                let _ = reject.call1(
                    &wasm_bindgen::JsValue::UNDEFINED,
                    &wasm_bindgen::JsValue::from_str(&msg),
                );
            },
        );
        error_req.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    });
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .map_err(|e| format!("IDB promise: {e:?}"))
}
