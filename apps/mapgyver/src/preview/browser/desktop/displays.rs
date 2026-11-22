use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use golden_core::application::ProductionRuntime;
use golden_io::{RevisionSubscription, WorkerNotifier};

use crate::{
    app::AppNode,
    screens::{
        catalog_runtime::{Source, display_options},
        host_inventory::project_displays,
        protocol::DisplayInfo,
    },
};

pub(super) struct Displays {
    active: Arc<[DisplayInfo]>,
    next_scan: Instant,
    source: Option<Source>,
    subscription: Option<RevisionSubscription>,
    needs_publish: bool,
    scan_error: Option<String>,
}

impl Displays {
    pub fn new() -> Self {
        Self {
            active: Arc::from([]),
            next_scan: Instant::now(),
            source: None,
            subscription: None,
            needs_publish: true,
            scan_error: None,
        }
    }

    pub fn deadline(&self) -> Instant {
        self.next_scan
    }

    pub fn active(&self) -> Arc<[DisplayInfo]> {
        self.active.clone()
    }

    pub fn refresh(
        &mut self,
        app: &tauri::AppHandle,
        runtime: &ProductionRuntime<AppNode>,
        notifier: &WorkerNotifier,
    ) -> Result<bool, String> {
        let now = Instant::now();
        if now >= self.next_scan {
            self.next_scan = now + Duration::from_secs(1);
            let active = match host_displays(app).and_then(project_displays) {
                Ok(displays) => {
                    self.scan_error = None;
                    displays
                }
                Err(error) => {
                    let message = error.to_string();
                    if self.scan_error.as_ref() != Some(&message) {
                        eprintln!("[mapgyver] desktop display catalog unavailable: {message}");
                        self.scan_error = Some(message);
                    }
                    Vec::new()
                }
            };
            if active != self.active.as_ref() {
                self.active = active.into();
                self.needs_publish = true;
            }
        }
        if self
            .source
            .as_ref()
            .is_none_or(|source| source.publisher.is_closed() || !source.guard.is_current())
        {
            let source = runtime
                .capture_app(Source::capture)
                .map_err(|error| error.to_string())??;
            self.subscription = Some(source.guard.subscribe(notifier.clone()));
            self.source = Some(source);
            self.needs_publish = true;
        }
        if self.needs_publish {
            let source = self.source.as_ref().expect("display source is current");
            source
                .publisher
                .publish(source.prepare_exact(self.active.clone(), &display_options(&self.active)))?;
            self.needs_publish = false;
            return Ok(true);
        }
        Ok(false)
    }
}

#[cfg(target_os = "macos")]
fn host_displays(app: &tauri::AppHandle) -> std::io::Result<Vec<golden_core::app::NativeDisplay>> {
    golden_core::app::macos_desktop_displays(app)
}

#[cfg(target_os = "linux")]
fn host_displays(_: &tauri::AppHandle) -> std::io::Result<Vec<golden_core::app::NativeDisplay>> {
    golden_core::app::x11_desktop_displays()
}
