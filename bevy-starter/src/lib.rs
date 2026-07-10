mod scene;

#[cfg(target_arch = "wasm32")]
pub mod laye_extern;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
const HOST_SURFACE: &str = "bevy-starter";

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
fn next_error_id() -> String {
    use std::cell::Cell;
    thread_local! {
        static COUNTER: Cell<u64> = const { Cell::new(0) };
    }
    COUNTER.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        format!("err-starter-{n}")
    })
}

#[cfg(target_arch = "wasm32")]
fn now_ms() -> String {
    (js_sys::Date::now() as u64).to_string()
}

/// Route bevy-starter errors through laye-p2p's sacred overlay so one
/// render path serves every layer per ERROR.md. No local #errors div,
/// no per-surface reinvention.
#[cfg(target_arch = "wasm32")]
fn emit_typed(severity: laye_error::Severity, region: &str, title: &str, why: impl Into<String>) {
    let err = laye_error::Error {
        id: next_error_id(),
        severity,
        context: laye_error::Context {
            surface: HOST_SURFACE.to_string(),
            region: Some(region.to_string()),
            anchor: None,
        },
        title: title.to_string(),
        why: why.into(),
        trace: Vec::new(),
        raw: None,
        at: now_ms(),
        source: Some("rust-ffi".to_string()),
        ffi_call: None,
        location: None,
        js_stack: None,
        raw_stderr: None,
        requires_reload: false,
    };
    match serde_json::to_string(&err) {
        Ok(json) => laye_extern::emit_error(&json),
        Err(_) => {
            // If serialization itself fails, we've lost the pipeline.
            // Nothing else to do — this is the terminal case.
            web_sys::console::error_1(&wasm_bindgen::JsValue::from_str(
                "bevy-starter: could not serialize its own Error for laye emit",
            ));
        }
    }
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
            let severity = match level {
                Level::ERROR => laye_error::Severity::Error,
                Level::WARN => laye_error::Severity::Warn,
                _ => return,
            };
            let target = event.metadata().target();
            let mut v = MessageVisitor::default();
            event.record(&mut v);
            emit_typed(severity, "bevy-log", target, v.message);
        }
    }

    Some(Box::new(WasmErrorLayer))
}

#[cfg(target_arch = "wasm32")]
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let payload = info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        let err = laye_error::Error {
            id: next_error_id(),
            severity: laye_error::Severity::Panic,
            context: laye_error::Context {
                surface: HOST_SURFACE.to_string(),
                region: Some("wasm-panic".to_string()),
                anchor: None,
            },
            title: "bevy-starter panicked".to_string(),
            why: format!("panic: {msg}"),
            trace: Vec::new(),
            raw: None,
            at: now_ms(),
            source: Some("rust-panic".to_string()),
            ffi_call: None,
            location,
            js_stack: None,
            raw_stderr: None,
            requires_reload: true,
        };
        if let Ok(json) = serde_json::to_string(&err) {
            laye_extern::emit_error(&json);
        }
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
    {
        install_panic_hook();
        // laye-p2p was already loaded + init'd by index.html before
        // this wasm was fetched. window.laye.self_peer_id() is safe now.
        let peer_id = laye_extern::self_peer_id();
        set_status(&format!(
            "laye sibling up — peer_id {}… — starting scene",
            &peer_id[..8.min(peer_id.len())]
        ));
    }
    scene::build_and_run_app();
}
