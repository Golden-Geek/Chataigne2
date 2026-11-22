//! Physical Screen demand for the portable desktop presentation worker.

use std::{collections::HashMap, io, num::NonZeroUsize, sync::mpsc, time::Duration};

use golden_core::{
    app::{NativeOutputBounds, NativePresentation, NativePresentationTask, open_desktop_output_window},
    application::ProductionRuntime,
    transport::BrowserPreviews,
};
use golden_io::{RetirementPool, RevisionGuard, RevisionSubscription, WorkerNotifier};
use golden_video::{OutputId, wgpu_backend::WindowOutputs};
use tauri::Manager;

#[cfg(target_os = "macos")]
mod displays;

use crate::{
    app::AppNode,
    media::rendering::{portable_channels, portable_screen},
    preview::{capture::CapturedPreview, protocol::PreviewSource},
    screens::{
        protocol::ScreenId,
        rendering::{MAX_SCREEN_CATALOG, MAX_SCREEN_OUTPUTS, ResolvedTarget, capture_outputs},
    },
};

const CHECK: Duration = Duration::from_millis(25);

pub(crate) fn presentation(
    runtime: ProductionRuntime<AppNode>,
    previews: BrowserPreviews,
) -> NativePresentation<tauri::Wry> {
    NativePresentation::for_output_windows(move |window| {
        #[cfg(target_os = "linux")]
        let window_targets = golden_core::app::supports_desktop_rectangle_outputs(window);
        #[cfg(not(target_os = "linux"))]
        let window_targets = true;
        let app = window.app_handle().clone();
        Ok(NativePresentationTask::new(move |stop| {
            let desktop = (window_targets
                && (!cfg!(debug_assertions)
                    || std::env::var("MAPGYVER_AUTOMATION_DISABLE_DISPLAY_OUTPUTS").as_deref() != Ok("1")))
            .then(|| Desktop::new(app));
            let pool = RetirementPool::new(NonZeroUsize::MIN);
            let worker = super::start_worker(runtime, previews, &pool, desktop)?;
            while !stop.wait_timeout(CHECK) && !pool.wait_for_idle(Duration::ZERO) {}
            drop(worker);
            while !pool.wait_for_idle(CHECK) {}
            Ok(())
        }))
    })
}

pub(super) struct Desktop {
    app: tauri::AppHandle,
    windows: WindowOutputs,
    desired: Vec<(ScreenId, ResolvedTarget)>,
    #[cfg(target_os = "macos")]
    displays: displays::Displays,
    guards: Option<Vec<RevisionGuard>>,
    subscriptions: Vec<RevisionSubscription>,
    bindings: HashMap<ScreenId, Binding>,
    next: u64,
}

struct Binding {
    id: OutputId,
    target: ResolvedTarget,
}

impl Desktop {
    fn new(app: tauri::AppHandle) -> Self {
        Self {
            app,
            windows: WindowOutputs::default(),
            desired: Vec::new(),
            #[cfg(target_os = "macos")]
            displays: displays::Displays::new(),
            guards: None,
            subscriptions: Vec::new(),
            bindings: HashMap::new(),
            next: super::OUTPUT_SLOTS as u64,
        }
    }

    pub fn windows(&self) -> WindowOutputs {
        self.windows.clone()
    }

    #[cfg(target_os = "macos")]
    pub fn deadline(&self) -> std::time::Instant {
        self.displays.deadline()
    }

    pub fn refresh(&mut self, runtime: &ProductionRuntime<AppNode>, notifier: &WorkerNotifier) -> Result<bool, String> {
        #[cfg(target_os = "macos")]
        let mut changed = self.displays.refresh(&self.app, runtime, notifier)?;
        #[cfg(not(target_os = "macos"))]
        let mut changed = false;
        if changed
            || !self
                .guards
                .as_ref()
                .is_some_and(|guards| guards.iter().all(RevisionGuard::is_current))
        {
            #[cfg(target_os = "macos")]
            let displays = self.displays.active();
            #[cfg(not(target_os = "macos"))]
            let displays = [];
            let capture = runtime
                .capture_app(move |engine| capture_outputs(engine, MAX_SCREEN_CATALOG, MAX_SCREEN_OUTPUTS, &displays))
                .map_err(|error| error.to_string())??;
            self.desired = capture
                .outputs
                .into_iter()
                .filter_map(|output| match output.target {
                    target @ ResolvedTarget::Rectangle(_) => Some((output.screen, target)),
                    #[cfg(target_os = "macos")]
                    target @ ResolvedTarget::Fullscreen { .. } => Some((output.screen, target)),
                    _ => None,
                })
                .collect();
            self.subscriptions = capture
                .guards
                .iter()
                .map(|guard| guard.subscribe(notifier.clone()))
                .collect();
            self.guards = Some(capture.guards);
            changed = true;
        }
        self.bindings.retain(|screen, binding| {
            let keep = self
                .desired
                .iter()
                .any(|(id, target)| id == screen && *target == binding.target)
                && !self.windows.needs_reopen(binding.id);
            if !keep {
                self.windows.detach(binding.id);
                changed = true;
            }
            keep
        });
        Ok(changed)
    }

    pub fn has_outputs(&self) -> bool {
        !self.desired.is_empty()
    }

    pub fn sources(&self) -> impl Iterator<Item = PreviewSource> + '_ {
        self.desired
            .iter()
            .map(|(screen, _)| PreviewSource::Screen { screen: *screen })
    }

    pub fn bind(&mut self, capture: &mut CapturedPreview) -> Result<(), String> {
        let channels = portable_channels(&capture.channels);
        self.bindings.retain(|screen, binding| {
            let keep = capture
                .screens
                .iter()
                .any(|route| route.screen.id() == *screen && portable_screen(route, &channels));
            if !keep {
                self.windows.detach(binding.id);
            }
            keep
        });
        for route in &mut capture.screens {
            let screen = route.screen.id();
            let Some((_, target)) = self.desired.iter().find(|(id, _)| *id == screen) else {
                continue;
            };
            if !portable_screen(route, &channels) {
                continue;
            }
            route.extent = match target {
                ResolvedTarget::Rectangle(bounds) | ResolvedTarget::Fullscreen { bounds, .. } => bounds.extent,
                _ => unreachable!("desktop demand contains only window targets"),
            };
            let id = match self.bindings.get(&screen) {
                Some(binding) => binding.id,
                None => {
                    let target = target.clone();
                    let id = self.open(&target).map_err(|error| error.to_string())?;
                    self.bindings.insert(screen, Binding { id, target });
                    id
                }
            };
            route.output = Some(id);
        }
        Ok(())
    }

    fn open(&mut self, target: &ResolvedTarget) -> io::Result<OutputId> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| io::Error::other("output identity exhausted"))?;
        let id = OutputId::new(self.next).unwrap();
        let label = format!("mapgyver-output-{}", id.get());
        let title = "MapGyver Screen Output";
        let physical = |bounds: crate::screens::protocol::PhysicalRectangle| {
            NativeOutputBounds::new(bounds.left, bounds.top, bounds.extent.width(), bounds.extent.height())
        };
        let (window, extent) = match target {
            ResolvedTarget::Rectangle(bounds) => (
                open_desktop_output_window(&self.app, &label, title, physical(*bounds)?)?,
                bounds.extent,
            ),
            #[cfg(target_os = "macos")]
            ResolvedTarget::Fullscreen { bounds, display } => (
                golden_core::app::open_macos_fullscreen_output(&self.app, &label, title, display, physical(*bounds)?)?,
                bounds.extent,
            ),
            _ => return Err(io::Error::other("unsupported portable output target")),
        };
        let (send, receive) = mpsc::sync_channel(1);
        let windows = self.windows.clone();
        let setup = window.clone();
        if let Err(error) = window.run_on_main_thread(move || {
            #[cfg(target_os = "linux")]
            if let Err(error) = golden_core::app::realize_linux_output_window(&setup) {
                let _ = setup.destroy();
                let _ = send.send(Err(error.to_string()));
                return;
            }
            let close = setup.clone();
            let result = windows.attach(
                id,
                setup.clone(),
                extent,
                move || setup.show().map_err(|error| error.to_string()),
                move || {
                    let _ = close.destroy();
                },
            );
            let failed = result.is_err();
            if send.send(result).is_err() || failed {
                windows.detach(id);
            }
        }) {
            let _ = window.destroy();
            return Err(io::Error::other(error));
        }
        let result = receive
            .recv_timeout(Duration::from_secs(5))
            .map_err(io::Error::other)
            .and_then(|result| result.map_err(io::Error::other));
        if let Err(error) = result {
            self.windows.detach(id);
            return Err(error);
        }
        Ok(id)
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        for binding in self.bindings.values() {
            self.windows.detach(binding.id);
        }
    }
}
