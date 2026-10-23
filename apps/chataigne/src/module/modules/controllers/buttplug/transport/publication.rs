use super::*;
use crate::app::module::script_api::{NativeCallbackDraft, NativeCallbackPayload, NativeCallbackPreparer};
use golden_core::parameter::ParameterEnumOptions;
use std::{sync::Mutex, time::Duration};

pub(in super::super) struct Catalog {
    pub(in super::super) options: ParameterEnumOptions,
    pub(in super::super) selection: String,
}
pub(in super::super) struct Publication {
    pub(in super::super) event: ButtplugWorkerEvent,
    pub(in super::super) callback: Option<NativeCallbackPayload>,
    pub(in super::super) catalog: Option<Catalog>,
    pub(in super::super) guards: [RevisionGuard; 3],
}
impl Publication {
    pub(in super::super) fn current(&self) -> bool {
        self.guards.iter().all(RevisionGuard::is_current)
    }
    pub(in super::super) fn devices(&self) -> &[ButtplugDeviceInfo] {
        match &self.event {
            ButtplugWorkerEvent::Devices(devices) => devices,
            _ => &[],
        }
    }
}

fn domain_pool() -> Result<PayloadPool, String> {
    static POOL: OnceLock<Mutex<Option<PayloadPool>>> = OnceLock::new();
    let mut cell = POOL
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if cell.is_none() {
        *cell = Some(PayloadPool::new("buttplug-catalogs", 128, 64 * 1024 * 1024).map_err(|error| error.to_string())?);
    }
    Ok(cell.as_ref().unwrap().clone())
}
pub(super) struct Publisher<'a> {
    ctx: &'a worker::Context,
    pool: &'a PayloadPool,
    shared: &'a Shared,
    latest: Option<RetainedPayload<Publication>>,
    selection: String,
    callbacks: Option<NativeCallbackPreparer>,
}
impl<'a> Publisher<'a> {
    pub(super) fn new(ctx: &'a worker::Context, pool: &'a PayloadPool, shared: &'a Shared) -> Self {
        Self {
            ctx,
            pool,
            shared,
            latest: None,
            selection: String::new(),
            callbacks: None,
        }
    }
    #[cfg(test)]
    pub(super) fn using_callbacks(mut self, callbacks: NativeCallbackPreparer) -> Self {
        self.callbacks = Some(callbacks);
        self
    }
    fn pause(&self) -> bool {
        if !self.shared.pressure.swap(true, Ordering::AcqRel) {
            self.shared.notifier.notify();
        }
        !self.ctx.cancellation().wait(Duration::from_millis(10))
    }
    pub(super) fn recovering(&mut self, message: String) -> bool {
        self.publish(ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Recovering {
            message: admission::diagnostic(message),
        }))
    }
    pub(super) fn all(&mut self, input: Vec<ButtplugWorkerEvent>) -> bool {
        if input.len() > 4 {
            self.recovering("Buttplug provider event turn exceeds four records".into());
            return false;
        }
        for event in input {
            if !self.publish(event) {
                return false;
            }
        }
        true
    }
    pub(super) fn selection_changed(&mut self) -> bool {
        let selection = self
            .shared
            .selection
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone();
        if selection == self.selection {
            return true;
        }
        let Some(last) = &self.latest else {
            return true;
        };
        self.publish(ButtplugWorkerEvent::Devices(last.devices().to_vec()))
    }
    pub(super) fn publish(&mut self, mut event: ButtplugWorkerEvent) -> bool {
        if let Err(error) = admission::event(&event) {
            event = ButtplugWorkerEvent::Error(error);
        }
        let reservation = loop {
            match self.pool.try_reserve(EVENT_WEIGHT) {
                Ok(value) => break value,
                Err(_) => {
                    if !self.pause() {
                        return false;
                    }
                }
            }
        };
        let callback = match &event {
            ButtplugWorkerEvent::DeviceAdded(device) => Some((
                super::super::BUTTPLUG_DEVICE_ADDED_CALLBACK,
                vec![super::super::device_arg(device)],
            )),
            ButtplugWorkerEvent::DeviceRemoved(device) => Some((
                super::super::BUTTPLUG_DEVICE_REMOVED_CALLBACK,
                vec![super::super::device_arg(device)],
            )),
            ButtplugWorkerEvent::ScanningFinished => {
                Some((super::super::BUTTPLUG_SCANNING_FINISHED_CALLBACK, Vec::new()))
            }
            _ => None,
        };
        let callback = if let Some((name, args)) = callback {
            let draft = match NativeCallbackDraft::new(name, args) {
                Ok(value) => value,
                Err(_) => return false,
            };
            Some(loop {
                if self.callbacks.is_none() {
                    self.callbacks = NativeCallbackPreparer::new().ok();
                }
                if let Some(value) = self.callbacks.as_ref().and_then(|preparer| preparer.try_retain(&draft)) {
                    break value;
                }
                if !self.pause() {
                    return false;
                }
            })
        } else {
            None
        };
        let devices = match &event {
            ButtplugWorkerEvent::Devices(devices) => Some(devices.as_slice()),
            ButtplugWorkerEvent::Status(ButtplugConnectionStatus::Recovering { .. }) => Some(&[][..]),
            _ => None,
        };
        let catalog = if let Some(devices) = devices {
            let selection = self
                .shared
                .selection
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            let mut options = super::super::buttplug_device_options(devices);
            if super::super::specific_device_selected(&selection)
                && !options.iter().any(|option| option.variant_id == selection)
            {
                options.push(super::super::missing_device_option(&selection));
            }
            let mut options: ParameterEnumOptions = options.into();
            if options.retained_bytes() > 512 * 1024 {
                return false;
            }
            if let Some(previous) = self
                .latest
                .as_ref()
                .and_then(|record| record.catalog.as_ref())
                .filter(|previous| previous.options == options)
            {
                options = previous.options.clone();
            } else {
                loop {
                    if let Ok(pool) = domain_pool() {
                        if pool
                            .try_retain_arc(options.shared().clone(), options.retained_bytes())
                            .is_ok()
                        {
                            break;
                        }
                    }
                    if !self.pause() {
                        return false;
                    }
                }
            }
            Some(Catalog { options, selection })
        } else {
            None
        };
        let prepared = Publication {
            event,
            callback,
            catalog,
            guards: [
                self.shared.actor.clone(),
                self.shared.native.guard_at(0),
                self.shared.session.guard_at(self.shared.session.current()),
            ],
        };
        let record = match reservation.retain(prepared, EVENT_WEIGHT) {
            Ok(value) => value,
            Err(_) => return false,
        };
        if self.ctx.publish_weighted(record.clone(), EVENT_WEIGHT).is_err() {
            return false;
        }
        if let Some(catalog) = &record.catalog {
            self.selection = catalog.selection.clone();
            self.latest = Some(record);
        }
        if self.shared.pressure.swap(false, Ordering::AcqRel) {
            self.shared.notifier.notify();
        }
        true
    }
}
