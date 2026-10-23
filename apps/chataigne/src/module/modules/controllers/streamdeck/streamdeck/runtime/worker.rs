use super::*;
use crate::app::module::script_api::NativeCallbackPreparer;
use golden_io::{PendingSendError, ReconnectBackoff};
use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

struct PendingInput {
    draft: Draft,
    guard: RevisionGuard,
    selection: RevisionGuard,
    requested: String,
}
struct State {
    frame: Option<RetainedPayload<Frame>>,
    requested: Option<String>,
    connected: Option<DeviceInfo>,
    devices: Vec<DeviceInfo>,
    options: ParameterEnumOptions,
    status: Option<Arc<Status>>,
    last_status: Option<Arc<Status>>,
    input: VecDeque<PendingInput>,
    publication: Option<RetainedPayload<Publication>>,
    session: RevisionSource,
    session_revision: u64,
    retry_at: Instant,
    discover_at: Instant,
    backoff: ReconnectBackoff,
    rendered: Vec<Option<KeyVisual>>,
    brightness: Option<u8>,
    asset_retry: [Option<Instant>; MAX_VISUALS],
    asset_error: Option<String>,
}
impl State {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            frame: None,
            requested: None,
            connected: None,
            devices: Vec::new(),
            options: Vec::new().into(),
            status: None,
            last_status: None,
            input: VecDeque::with_capacity(MAX_KEYS),
            publication: None,
            session: RevisionSource::default(),
            session_revision: 0,
            retry_at: now,
            discover_at: now,
            backoff: ReconnectBackoff::new(Duration::from_millis(250), Duration::from_secs(5)),
            rendered: vec![None; MAX_VISUALS],
            brightness: None,
            asset_retry: [None; MAX_VISUALS],
            asset_error: None,
        }
    }
    fn status(&mut self, selection: &str, message: Option<String>) -> Result<(), String> {
        let mut options = vec![
            super::super::enum_option(super::super::AUTO_DEVICE_VARIANT, "Auto", 0),
            super::super::enum_option(super::super::NO_DEVICE_VARIANT, "No Device", 1),
        ];
        for (index, device) in self.devices.iter().enumerate() {
            device.validate()?;
            options.push(super::super::enum_option(
                &device.serial,
                &device.product,
                index as i32 + 2,
            ));
        }
        if selection != super::super::AUTO_DEVICE_VARIANT
            && selection != super::super::NO_DEVICE_VARIANT
            && !options.iter().any(|o| o.variant_id == selection)
        {
            options.push(super::super::enum_option(
                selection,
                &format!("Missing: {selection}"),
                66,
            ));
        }
        let options: ParameterEnumOptions = options.into();
        if options.len() > 67 || options.retained_bytes() > STATUS_WEIGHT {
            return Err("Stream Deck device choices exceed 67 rows / 256 KiB".into());
        }
        if self.options != options {
            self.options = options;
        }
        let connected = self.connected.is_some();
        let message = message.map(bounded_error);
        if self.last_status.as_ref().is_some_and(|s| {
            s.selection == selection && s.options == self.options && s.connected == connected && s.message == message
        }) {
            return Ok(());
        }
        let status = Arc::new(Status {
            selection: selection.into(),
            options: self.options.clone(),
            connected,
            message,
        });
        self.last_status = Some(status.clone());
        self.status = Some(status);
        Ok(())
    }
    fn drained(&self) -> bool {
        self.input.is_empty() && self.status.is_none() && self.publication.is_none()
    }
    fn reset_feedback(&mut self) {
        self.rendered.fill(None);
        self.brightness = None;
        self.asset_retry.fill(None);
        self.asset_error = None;
    }
}
pub(super) fn run<D: Driver>(mut owned: Owned<D>, resources: &Resources, guards: &[RevisionGuard; 2]) {
    if let Err(error) = work(&mut owned, resources, guards) {
        failure(&owned.stopped.shared, error);
    }
}
fn work<D: Driver>(owned: &mut Owned<D>, resources: &Resources, guards: &[RevisionGuard; 2]) -> Result<(), String> {
    let Resources {
        sources: pool,
        domains,
        callbacks,
    } = resources;
    let mut state = State::new();
    while !owned.ctx.cancellation().is_cancelled() {
        let frame = owned
            .stopped
            .shared
            .desired
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if !frame.selection_guard.is_current() {
            owned.ctx.cancellation().wait(Duration::from_millis(8));
            continue;
        }
        if state
            .frame
            .as_ref()
            .is_none_or(|previous| !std::ptr::eq::<Frame>(&**previous, &*frame))
        {
            state.frame = Some(frame.clone());
            state.asset_retry.fill(None);
            state.asset_error = None;
        }
        if state.requested.as_deref() != Some(frame.selection.as_str()) {
            state.input.clear();
            state.publication = None;
            state.status = None;
            state.last_status = None;
            state.session_revision = state.session.advance();
            state.requested = Some(frame.selection.clone());
            state.connected = None;
            state.reset_feedback();
            state.status(&frame.selection, None)?;
            // Same owner throughout reconfiguration; no new thread while an SDK call blocks.
            owned.driver.disconnect()?;
            state.discover_at = Instant::now();
            state.retry_at = Instant::now();
            state.backoff.reset();
        }
        flush(owned, &mut state, pool, domains, callbacks, guards)?;
        let mut device_error = None;
        if let Some(key_count) = state
            .connected
            .as_ref()
            .filter(|info| info.feedback && frame.draw_guard.is_current())
            .map(|info| info.key_count)
        {
            if state.brightness != Some(frame.brightness) {
                match owned.driver.brightness(frame.brightness) {
                    Ok(()) => state.brightness = Some(frame.brightness),
                    Err(error) => device_error = Some(error),
                }
            }
            if state.asset_retry.iter().all(Option::is_none) {
                state.asset_error = None;
            }
            let count = frame.visuals.len().min(key_count);
            let mut rendered = 0;
            for index in 0..count {
                if device_error.is_some() || rendered == 4 || !frame.draw_guard.is_current() {
                    break;
                }
                let visual = &frame.visuals[index];
                if state.rendered[index].as_ref() == Some(visual)
                    || state.asset_retry[index].is_some_and(|at| Instant::now() < at)
                {
                    continue;
                }
                rendered += 1;
                match owned.driver.render(index, visual) {
                    Ok(()) => {
                        state.rendered[index] = Some(visual.clone());
                        state.asset_retry[index] = None;
                    }
                    Err(RenderError::Asset(error)) => {
                        state.asset_error = Some(bounded_error(error));
                        state.asset_retry[index] = Some(Instant::now() + Duration::from_secs(2));
                    }
                    Err(RenderError::Device(error)) => device_error = Some(error),
                }
            }
        }
        // Input/callback/catalog pressure never blocks feedback. Stop new reads/state
        // transitions until the retained ordered prefix has publication capacity again.
        if let Some(error) = device_error {
            return Err(error);
        }
        if state.drained() {
            if Instant::now() >= state.discover_at {
                state.discover_at = Instant::now() + Duration::from_secs(2);
                match owned.driver.discover() {
                    Ok(devices) => {
                        if devices.capacity() > 64 {
                            return Err("Stream Deck discovery exceeds 64 records".into());
                        }
                        for device in &devices {
                            device.validate()?;
                        }
                        state.devices = devices;
                        state.status(&frame.selection, state.asset_error.clone())?;
                    }
                    Err(error) => state.status(&frame.selection, Some(error))?,
                }
            } else if state.connected.is_none() && Instant::now() >= state.retry_at {
                let serial = if frame.selection == super::super::AUTO_DEVICE_VARIANT {
                    state.devices.first().map(|d| d.serial.clone())
                } else if frame.selection == super::super::NO_DEVICE_VARIANT {
                    None
                } else {
                    Some(frame.selection.clone())
                };
                state.retry_at = state.backoff.schedule(Instant::now());
                if let Some(serial) = serial {
                    match owned.driver.connect(&serial) {
                        Ok(info) => {
                            info.validate()?;
                            if info.serial != serial {
                                return Err("Stream Deck provider connected a different serial".into());
                            }
                            state.session_revision = state.session.advance();
                            state.connected = Some(info);
                            state.reset_feedback();
                            state.backoff.reset();
                            state.status(&frame.selection, None)?;
                        }
                        Err(error) => state.status(&frame.selection, Some(error))?,
                    }
                }
            } else if state.connected.is_some() {
                let input = owned.driver.poll()?;
                // A driver can block despite a zero-timeout report API. Capture the
                // route when the report returns, while cancelling a retired serial.
                let observed = owned
                    .stopped
                    .shared
                    .desired
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                if !observed.selection_guard.is_current() || observed.selection != frame.selection {
                    continue;
                }
                if input.capacity() > MAX_KEYS {
                    return Err("Stream Deck input turn exceeds 64 records".into());
                }
                for input in input {
                    let (index, _) = input.parts();
                    let route = observed
                        .routes
                        .get(index)
                        .ok_or("Stream Deck report index exceeds captured routes")?
                        .clone();
                    state.input.push_back(PendingInput {
                        draft: Draft::new(input, route, Some(observed.clone()))?,
                        guard: state.session.reader().guard_at(state.session_revision),
                        selection: observed.selection_guard.clone(),
                        requested: observed.selection.clone(),
                    });
                }
                if state
                    .last_status
                    .as_ref()
                    .is_some_and(|s| s.message != state.asset_error)
                {
                    state.status(&frame.selection, state.asset_error.clone())?;
                }
            }
        }
        flush(owned, &mut state, pool, domains, callbacks, guards)?;
        if owned.ctx.cancellation().wait(Duration::from_millis(8)) {
            break;
        }
    }
    Ok(())
}
fn flush<D>(
    owned: &Owned<D>,
    state: &mut State,
    pool: &PayloadPool,
    domains: &PayloadPool,
    callbacks: &NativeCallbackPreparer,
    guards: &[RevisionGuard],
) -> Result<(), String> {
    let mut pressure = false;
    for _ in 0..16 {
        if state.publication.is_none() {
            let reservation = match pool.try_reserve(INPUT_WEIGHT) {
                Ok(reservation) => reservation,
                Err(_) => {
                    pressure = !state.drained();
                    break;
                }
            };
            if let Some(status) = &state.status {
                if domains
                    .try_retain_arc(status.options.shared().clone(), status.options.retained_bytes())
                    .is_err()
                {
                    pressure = true;
                    break;
                }
                let Ok(status) = pool.try_retain_arc(status.clone(), STATUS_WEIGHT) else {
                    pressure = true;
                    break;
                };
                let frame = owned
                    .stopped
                    .shared
                    .desired
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let mut fences = guards.to_vec();
                fences.push(frame.selection_guard.clone());
                let selection = status.selection.clone();
                state.publication = Some(
                    reservation
                        .retain(
                            Publication {
                                event: Event::Status(status),
                                guards: fences,
                                selection,
                            },
                            INPUT_WEIGHT,
                        )
                        .map_err(|e| e.to_string())?,
                );
                state.status = None;
            } else if let Some(input) = state.input.front() {
                let Some(prepared) = input.draft.finish(callbacks) else {
                    pressure = true;
                    break;
                };
                let mut fences = guards.to_vec();
                fences.push(input.selection.clone());
                fences.push(input.guard.clone());
                state.publication = Some(
                    reservation
                        .retain(
                            Publication {
                                event: Event::Input(prepared),
                                guards: fences,
                                selection: input.requested.clone(),
                            },
                            INPUT_WEIGHT,
                        )
                        .map_err(|e| e.to_string())?,
                );
                state.input.pop_front();
            } else {
                break;
            }
        }
        match owned
            .ctx
            .try_publish_weighted(state.publication.take().unwrap(), INPUT_WEIGHT)
        {
            Ok(()) => {}
            Err(PendingSendError::Full(row)) => {
                state.publication = Some(row);
                pressure = true;
                break;
            }
            Err(PendingSendError::Disconnected(_)) => return Ok(()),
        }
    }
    if owned.stopped.shared.pressure.swap(pressure, Ordering::AcqRel) != pressure {
        owned.stopped.shared.notifier.notify();
    }
    Ok(())
}
