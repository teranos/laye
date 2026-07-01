use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bevy_log::{
    BoxedLayer,
    tracing::{self, Subscriber},
    tracing_subscriber::Layer,
};
use std::sync::Mutex;

pub use bevy_log::tracing::Level;

pub const ERROR_LOG_CAP: usize = 200;

pub static PANIC_QUEUE: Mutex<Vec<String>> = Mutex::new(Vec::new());
pub static LOG_QUEUE: Mutex<Vec<(Severity, String)>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Note,
    Warn,
    Error,
}

pub struct ErrorEntry {
    pub severity: Severity,
    pub message: String,
}

#[derive(Resource, Default)]
pub struct ErrorLog(pub Vec<ErrorEntry>);

impl ErrorLog {
    pub fn emit(&mut self, severity: Severity, message: impl Into<String>) {
        self.0.push(ErrorEntry {
            severity,
            message: message.into(),
        });
        if self.0.len() > ERROR_LOG_CAP {
            let drop = self.0.len() - ERROR_LOG_CAP;
            self.0.drain(0..drop);
        }
    }
}

pub trait Metrics: Send + Sync {
    fn gauge(&self, name: &str, value: f64);
    fn counter(&self, name: &str, delta: u64);
}

pub struct StdoutSink;

impl Metrics for StdoutSink {
    fn gauge(&self, name: &str, value: f64) {
        bevy_log::tracing::info!(metric = name, value, kind = "gauge");
    }
    fn counter(&self, name: &str, delta: u64) {
        bevy_log::tracing::info!(metric = name, delta, kind = "counter");
    }
}

#[derive(Resource)]
pub struct MetricsSink(pub Box<dyn Metrics>);

pub struct ObservabilityPlugin;

impl Plugin for ObservabilityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ErrorLog::default());
        app.insert_resource(MetricsSink(Box::new(StdoutSink)));
        app.add_systems(Update, (drain_panics, drain_logs));
    }
}

/// Install via `LogPlugin { custom_layer: bevy_observability::install_capture_layer, ..default() }`.
pub fn install_capture_layer(_app: &mut App) -> Option<BoxedLayer> {
    Some(Box::new(CaptureLayer))
}

/// Sets the panic hook to write into `PANIC_QUEUE`. Call once before
/// `App::run()`. Chains the previous hook so other consumers still see
/// the panic.
pub fn install_panic_hook() {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let formatted = format!("{info}");
        let mut q = PANIC_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
        q.push(formatted);
        prev(info);
    }));
}

// Known-noisy targets — dropped before they reach the overlay so
// real signal isn't buried under wgpu-hal texture-view heuristic
// chatter (emitted at `log::error!` inside wgpu-hal's WebGL2 path,
// fires per-draw, isn't an actual error). Events still reach the
// browser devtools console via bevy_log's default subscriber; only
// the in-app overlay filters them out.
//
// Prefix match against the effective target — for `log::error!()`
// forwarded through `tracing-log`, the crate target lives in the
// `log.target` field, not `metadata().target()` (which is hardcoded
// to "log" by tracing-log 0.2). See `EventVisitor` below.
pub const DROPPED_LOG_TARGETS: &[&str] = &["wgpu_hal::gles"];

pub(crate) fn should_drop_target(target: &str) -> bool {
    DROPPED_LOG_TARGETS.iter().any(|p| target.starts_with(p))
}

pub struct CaptureLayer;

impl<S: Subscriber> Layer<S> for CaptureLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: bevy_log::tracing_subscriber::layer::Context<'_, S>,
    ) {
        let severity = match *event.metadata().level() {
            tracing::Level::ERROR => Severity::Error,
            tracing::Level::WARN => Severity::Warn,
            _ => return,
        };
        let mut visitor = EventVisitor::default();
        event.record(&mut visitor);
        let metadata_target = event.metadata().target();
        let effective_target = visitor
            .log_target
            .as_deref()
            .unwrap_or(metadata_target);
        if should_drop_target(effective_target) {
            return;
        }
        let formatted = format!("{effective_target}: {}", visitor.message);
        let mut q = LOG_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
        q.push((severity, formatted));
    }
}

#[derive(Default)]
struct EventVisitor {
    message: String,
    log_target: Option<String>,
}

impl tracing::field::Visit for EventVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        match field.name() {
            "message" => self.message = format!("{value:?}"),
            "log.target" => self.log_target = Some(format!("{value:?}")),
            _ => {}
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = value.to_string(),
            "log.target" => self.log_target = Some(value.to_string()),
            _ => {}
        }
    }
}

pub fn drain_panics(mut log: ResMut<ErrorLog>) {
    let mut q = PANIC_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
    for msg in q.drain(..) {
        log.emit(Severity::Error, format!("PANIC: {msg}"));
    }
}

pub fn drain_logs(mut log: ResMut<ErrorLog>) {
    let mut q = LOG_QUEUE.lock().unwrap_or_else(|p| p.into_inner());
    for (sev, msg) in q.drain(..) {
        log.emit(sev, msg);
    }
}

#[cfg(test)]
mod tests {
    use super::should_drop_target;

    #[test]
    fn wgpu_hal_gles_is_dropped() {
        assert!(should_drop_target("wgpu_hal::gles"));
        assert!(should_drop_target("wgpu_hal::gles::device"));
        assert!(should_drop_target("wgpu_hal::gles::adapter"));
    }

    #[test]
    fn non_wgpu_targets_pass_through() {
        assert!(!should_drop_target("wgpu"));
        assert!(!should_drop_target("wgpu_hal"));
        assert!(!should_drop_target("wgpu_hal::vulkan"));
        assert!(!should_drop_target("bevy_render"));
        assert!(!should_drop_target("rave"));
        assert!(!should_drop_target("log"));
    }
}
