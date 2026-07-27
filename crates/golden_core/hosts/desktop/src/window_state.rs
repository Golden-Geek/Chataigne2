use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{Monitor, PhysicalPosition, PhysicalSize, Position, Runtime, Size, WebviewWindow, Window};

const WINDOW_STATE_VERSION: u32 = 1;

#[derive(Clone)]
pub(crate) struct WindowStatePersistence {
    path: PathBuf,
}

impl WindowStatePersistence {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn load(&self) -> Result<Option<StoredWindowState>, String> {
        if !self.path.exists() {
            return Ok(None);
        }

        let contents = std::fs::read_to_string(&self.path)
            .map_err(|err| format!("failed reading persisted window state: {err}"))?;
        let state: StoredWindowState =
            serde_json::from_str(&contents).map_err(|err| format!("failed parsing persisted window state: {err}"))?;

        if state.version == WINDOW_STATE_VERSION {
            Ok(Some(state))
        } else {
            Ok(None)
        }
    }

    fn save<R: Runtime>(&self, window: &Window<R>) -> Result<(), String> {
        let position = window
            .outer_position()
            .map_err(|err| format!("failed reading window position: {err}"))?;
        let size = window
            .outer_size()
            .map_err(|err| format!("failed reading window size: {err}"))?;
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        let monitor = window
            .current_monitor()
            .map_err(|err| format!("failed reading current monitor: {err}"))?
            .map(|monitor| StoredMonitor::from_monitor(&monitor));
        let maximized = window.is_maximized().unwrap_or(false);
        let state = StoredWindowState::from_window(position, size, monitor, maximized);

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| format!("failed creating window state directory: {err}"))?;
        }

        let contents =
            serde_json::to_string_pretty(&state).map_err(|err| format!("failed serializing window state: {err}"))?;
        std::fs::write(&self.path, contents).map_err(|err| format!("failed writing persisted window state: {err}"))
    }
}

pub(crate) fn save_window_state<R: Runtime>(window: &Window<R>, persistence: &WindowStatePersistence) {
    if let Err(err) = persistence.save(window) {
        eprintln!(
            "warning: failed to persist window state for window '{}': {err}",
            window.label()
        );
    }
}

pub(crate) fn restore_window_state<R: Runtime>(window: &WebviewWindow<R>, persistence: &WindowStatePersistence) {
    let state = match persistence.load() {
        Ok(Some(state)) => state,
        Ok(None) => return,
        Err(err) => {
            eprintln!("warning: {err}");
            return;
        }
    };

    let monitors = match window.available_monitors() {
        Ok(monitors) => monitors.iter().map(StoredMonitor::from_monitor).collect::<Vec<_>>(),
        Err(err) => {
            eprintln!("warning: failed reading available monitors: {err}");
            Vec::new()
        }
    };
    let primary_monitor = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| StoredMonitor::from_monitor(&monitor));
    let primary_monitor = primary_monitor.as_ref();
    let Some(bounds) = state.restore_bounds(&monitors, primary_monitor) else {
        return;
    };

    if let Err(err) = window.set_size(Size::Physical(PhysicalSize::new(bounds.size.width, bounds.size.height))) {
        eprintln!("warning: failed restoring window size: {err}");
    }
    if let Err(err) = window.set_position(Position::Physical(PhysicalPosition::new(
        bounds.position.x,
        bounds.position.y,
    ))) {
        eprintln!("warning: failed restoring window position: {err}");
    }
    if bounds.maximized
        && let Err(err) = window.maximize()
    {
        eprintln!("warning: failed restoring maximized window state: {err}");
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct StoredWindowState {
    pub(crate) version: u32,
    pub(crate) outer_position: StoredPosition,
    pub(crate) outer_size: StoredSize,
    pub(crate) local_position: StoredPosition,
    pub(crate) monitor: Option<StoredMonitor>,
    pub(crate) maximized: bool,
}

impl StoredWindowState {
    pub(crate) fn from_window(
        position: PhysicalPosition<i32>,
        size: PhysicalSize<u32>,
        monitor: Option<StoredMonitor>,
        maximized: bool,
    ) -> Self {
        let outer_position = StoredPosition::from_physical(position);
        let outer_size = StoredSize::from_physical(size);
        let local_position = monitor
            .as_ref()
            .map(|monitor| StoredPosition {
                x: outer_position.x.saturating_sub(monitor.position.x),
                y: outer_position.y.saturating_sub(monitor.position.y),
            })
            .unwrap_or(outer_position);

        Self {
            version: WINDOW_STATE_VERSION,
            outer_position,
            outer_size,
            local_position,
            monitor,
            maximized,
        }
    }

    pub(crate) fn restore_bounds(
        &self,
        monitors: &[StoredMonitor],
        primary_monitor: Option<&StoredMonitor>,
    ) -> Option<RestoredWindowBounds> {
        let target_monitor = self
            .monitor
            .as_ref()
            .and_then(|monitor| monitor.find_match(monitors))
            .or(primary_monitor)
            .or_else(|| monitors.first());

        let Some(target_monitor) = target_monitor else {
            return Some(RestoredWindowBounds {
                position: self.outer_position,
                size: self.outer_size,
                maximized: self.maximized,
            });
        };

        let size = self.outer_size.clamp_to(target_monitor.size);
        let proposed_position = StoredPosition {
            x: target_monitor.position.x.saturating_add(self.local_position.x),
            y: target_monitor.position.y.saturating_add(self.local_position.y),
        };

        Some(RestoredWindowBounds {
            position: proposed_position.clamp_window_origin(size, target_monitor),
            size,
            maximized: self.maximized,
        })
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Serialize)]
pub(crate) struct StoredMonitor {
    pub(crate) name: Option<String>,
    pub(crate) position: StoredPosition,
    pub(crate) size: StoredSize,
    pub(crate) scale_factor: f64,
}

impl StoredMonitor {
    fn from_monitor(monitor: &Monitor) -> Self {
        Self {
            name: monitor.name().cloned(),
            position: StoredPosition::from_physical(*monitor.position()),
            size: StoredSize::from_physical(*monitor.size()),
            scale_factor: monitor.scale_factor(),
        }
    }

    fn find_match<'a>(&self, monitors: &'a [StoredMonitor]) -> Option<&'a StoredMonitor> {
        if let Some(name) = self.name.as_deref() {
            if let Some(monitor) = monitors
                .iter()
                .find(|monitor| monitor.name.as_deref() == Some(name) && monitor.size == self.size)
            {
                return Some(monitor);
            }

            if let Some(monitor) = monitors.iter().find(|monitor| monitor.name.as_deref() == Some(name)) {
                return Some(monitor);
            }
        }

        monitors
            .iter()
            .find(|monitor| monitor.position == self.position && monitor.size == self.size)
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
pub(crate) struct StoredPosition {
    pub(crate) x: i32,
    pub(crate) y: i32,
}

impl StoredPosition {
    fn from_physical(position: PhysicalPosition<i32>) -> Self {
        Self {
            x: position.x,
            y: position.y,
        }
    }

    fn clamp_window_origin(self, size: StoredSize, monitor: &StoredMonitor) -> Self {
        Self {
            x: clamp_axis(
                self.x,
                monitor.position.x,
                monitor.position.x.saturating_add(u32_to_i32(monitor.size.width)),
                u32_to_i32(size.width),
            ),
            y: clamp_axis(
                self.y,
                monitor.position.y,
                monitor.position.y.saturating_add(u32_to_i32(monitor.size.height)),
                u32_to_i32(size.height),
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Serialize)]
pub(crate) struct StoredSize {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

impl StoredSize {
    fn from_physical(size: PhysicalSize<u32>) -> Self {
        Self {
            width: size.width,
            height: size.height,
        }
    }

    fn clamp_to(self, monitor_size: StoredSize) -> Self {
        Self {
            width: self.width.min(monitor_size.width),
            height: self.height.min(monitor_size.height),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RestoredWindowBounds {
    pub(crate) position: StoredPosition,
    pub(crate) size: StoredSize,
    pub(crate) maximized: bool,
}

fn clamp_axis(value: i32, min: i32, max_exclusive: i32, size: i32) -> i32 {
    let max = max_exclusive.saturating_sub(size);
    if max <= min {
        return min;
    }
    value.clamp(min, max)
}

fn u32_to_i32(value: u32) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}
