//! Stable physical-window leases outlive renderer sessions. Session leases prevent teardown
//! until actual GPU retirement, including failed reconstruction and project replacement.
use super::targets::Target;
use crate::screens::{protocol::ScreenId, rendering::ResolvedTarget};
use golden_core::app::{NativeOutputEvent, NativeOutputWindow};
use golden_io::WorkerNotifier;
use golden_video::{OutputId, native::OutputBinding};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(super) struct WindowLease {
    window: NativeOutputWindow,
    pub pressed: Arc<AtomicBool>,
}

#[derive(Default)]
pub(super) struct OutputWindows {
    entries: HashMap<(uuid::Uuid, ScreenId), Arc<WindowLease>>,
}

impl OutputWindows {
    /// Called after the previous GPU owner retires when bindings change. Arc leases also
    /// preserve lifetime on error/unwind, independent of local variable destruction order.
    pub fn retain(&mut self, targets: &[Target]) {
        self.entries.retain(|&(project, screen), _| {
            targets.iter().any(|target| {
                target.project == project
                    && target.screen == screen
                    && !matches!(target.kind, ResolvedTarget::Spout { .. })
            })
        });
    }

    pub fn update_bounds(&self, targets: &[Target]) -> Result<(), String> {
        for target in targets {
            if let Some(window) = self.entries.get(&(target.project, target.screen)) {
                let bounds = target
                    .window_bounds()?
                    .ok_or("window target changed without GPU retirement")?;
                if window.window.physical_bounds().map_err(|error| error.to_string())? != bounds {
                    window
                        .window
                        .set_physical_bounds(bounds)
                        .map_err(|error| error.to_string())?;
                    eprintln!(
                        "[mapgyver-output] bounds id={} hwnd={} physical={bounds:?}",
                        target.id.get(),
                        window.window.raw_handle()
                    );
                }
            }
        }
        Ok(())
    }

    pub fn binding(
        &mut self,
        target: &Target,
        notifier: WorkerNotifier,
    ) -> Result<(OutputBinding, Option<Arc<WindowLease>>), String> {
        if let ResolvedTarget::Spout { sender, .. } = &target.kind {
            return Ok((
                OutputBinding::Spout {
                    id: target.id,
                    name: sender.clone(),
                    descriptor: target.descriptor,
                },
                None,
            ));
        }
        let bounds = target.window_bounds()?.ok_or("native window bounds missing")?;
        let lease = match self.entries.entry((target.project, target.screen)) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.get().clone(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let pressed = Arc::new(AtomicBool::new(false));
                let signal = pressed.clone();
                let window = NativeOutputWindow::with_events(
                    &format!("MapGyver output {}", target.id.get()),
                    bounds,
                    move |event| {
                        if event == NativeOutputEvent::EscapePressed && !signal.swap(true, Ordering::AcqRel) {
                            notifier.notify();
                        }
                    },
                )
                .map_err(|error| error.to_string())?;
                let lease = Arc::new(WindowLease { window, pressed });
                eprintln!(
                    "[mapgyver-output] window id={} hwnd={} physical={bounds:?}",
                    target.id.get(),
                    lease.window.raw_handle()
                );
                entry.insert(lease.clone());
                lease
            }
        };
        if lease.window.physical_bounds().map_err(|error| error.to_string())? != bounds {
            lease
                .window
                .set_physical_bounds(bounds)
                .map_err(|error| error.to_string())?;
        }
        // SAFETY: the renderer owner retains this lease through actual GPU retirement.
        let window = unsafe { golden_video::native::WindowTarget::from_raw(lease.window.raw_handle()) }
            .map_err(|error| error.to_string())?;
        Ok((
            OutputBinding::Window {
                id: target.id,
                window,
                extent: target.descriptor.extent,
            },
            Some(lease),
        ))
    }
}

pub(super) fn escape_targets(
    targets: &[Target],
    windows: &[(OutputId, Arc<WindowLease>)],
) -> Vec<(Target, Arc<AtomicBool>)> {
    targets
        .iter()
        .filter_map(|target| {
            let (_, lease) = windows.iter().find(|(id, _)| *id == target.id)?;
            Some((target.clone(), lease.pressed.clone()))
        })
        .collect()
}
