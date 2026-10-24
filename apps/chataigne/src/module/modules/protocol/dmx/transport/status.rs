//! Stream health is bounded latest state. Actor consumption never gates the stream clock.

use super::{DmxWorkerEvent, Shared, diagnostic};

#[derive(Default)]
pub(super) struct Status {
    ready: bool,
    warning: Option<String>,
    input_lost: Option<bool>,
    error: Option<String>,
}

impl Status {
    fn publish(&mut self, event: DmxWorkerEvent) {
        if self.error.is_some() {
            return;
        }
        match event {
            DmxWorkerEvent::Ready => self.ready = true,
            DmxWorkerEvent::Warning(message) => self.warning = Some(diagnostic(message)),
            DmxWorkerEvent::InputLost => self.input_lost = Some(true),
            DmxWorkerEvent::InputRecovered => self.input_lost = Some(false),
            DmxWorkerEvent::Error(message) => {
                // A terminal error supersedes older health, but cannot be overwritten by it.
                *self = Self {
                    error: Some(diagnostic(message)),
                    ..Self::default()
                };
            }
            #[cfg(test)]
            DmxWorkerEvent::Frame(_) => unreachable!("frames have an independent latest-state slot"),
        }
    }

    pub(super) fn has_pending(&self) -> bool {
        self.ready || self.warning.is_some() || self.input_lost.is_some() || self.error.is_some()
    }

    pub(super) fn take(&mut self) -> Option<DmxWorkerEvent> {
        if let Some(error) = self.error.take() {
            return Some(DmxWorkerEvent::Error(error));
        }
        if std::mem::take(&mut self.ready) {
            return Some(DmxWorkerEvent::Ready);
        }
        if let Some(warning) = self.warning.take() {
            return Some(DmxWorkerEvent::Warning(warning));
        }
        self.input_lost.take().map(|lost| {
            if lost {
                DmxWorkerEvent::InputLost
            } else {
                DmxWorkerEvent::InputRecovered
            }
        })
    }
}

impl Shared {
    pub(super) fn publish_status(&self, event: DmxWorkerEvent) {
        self.status
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .publish(event);
        self.wake.notify();
    }
}

#[cfg(test)]
mod tests;
