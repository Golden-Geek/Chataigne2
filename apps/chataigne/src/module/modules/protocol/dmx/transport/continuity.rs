//! Protocol state outlives sockets. Only native workers lock it or submit datagrams.

use super::{DmxFrame, DmxProtocol, DmxTransportConfig, NATIVE_LIFETIME_CAPACITY, output::ClockedOutput};
use std::{
    net::SocketAddr,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

#[cfg(test)]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct StreamKey(DmxProtocol, [u8; 16], u16);

#[derive(Default)]
struct Gate {
    epoch: AtomicU64,
    active: AtomicU64,
    emit: Mutex<()>,
}

struct Stream {
    key: StreamKey,
    initial_done: AtomicU64,
    revision: AtomicU64,
    state: Mutex<Option<State>>,
}

struct State {
    output: ClockedOutput,
    confirmed: Option<DmxFrame>,
    origin: Instant,
    epoch: u64,
    revision: u64,
}

/// App-owned continuity; claiming an I/O lifetime never locks native protocol state.
#[derive(Default)]
pub(crate) struct OutputContinuity {
    gate: Arc<Gate>,
    current: Option<Arc<Stream>>,
    retiring: Vec<Weak<Stream>>,
}

impl OutputContinuity {
    pub(super) fn claim(&mut self, config: &DmxTransportConfig) -> OutputSession {
        let key = StreamKey(config.protocol, config.source_id, config.universe);
        self.retiring.retain(|stream| stream.strong_count() != 0);
        let stream = self
            .current
            .as_ref()
            .filter(|stream| stream.key == key)
            .cloned()
            .or_else(|| {
                self.retiring
                    .iter()
                    .filter_map(Weak::upgrade)
                    .find(|stream| stream.key == key)
            })
            .unwrap_or_else(|| {
                Arc::new(Stream {
                    key,
                    initial_done: AtomicU64::new(0),
                    revision: AtomicU64::new(1),
                    state: Mutex::new(None),
                })
            });
        if let Some(previous) = self.current.replace(stream.clone()) {
            if !Arc::ptr_eq(&previous, &stream)
                && !self.retiring.iter().any(|old| old.ptr_eq(&Arc::downgrade(&previous)))
            {
                // At most sixteen native lifetimes can retain older streams; weak entries
                // never keep protocol storage alive after their native owner leaves.
                if self.retiring.len() == NATIVE_LIFETIME_CAPACITY {
                    self.retiring.remove(0);
                }
                self.retiring.push(Arc::downgrade(&previous));
            }
        }
        let epoch = self.gate.epoch.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
        let revision = stream.revision.load(Ordering::Acquire);
        OutputSession {
            gate: self.gate.clone(),
            stream,
            epoch,
            revision,
            config: config.clone(),
        }
    }

    pub(crate) fn restart_from_authored(&mut self) {
        self.gate.epoch.fetch_add(1, Ordering::AcqRel);
        for stream in self
            .current
            .iter()
            .cloned()
            .chain(self.retiring.iter().filter_map(Weak::upgrade))
        {
            stream.revision.fetch_add(1, Ordering::AcqRel);
        }
    }
}

#[derive(Clone)]
pub(super) struct OutputSession {
    gate: Arc<Gate>,
    stream: Arc<Stream>,
    epoch: u64,
    revision: u64,
    config: DmxTransportConfig,
}

impl OutputSession {
    fn current(&self) -> bool {
        self.gate.active.load(Ordering::Acquire) == self.epoch
    }

    fn activate(&self) -> Result<(), String> {
        if self.current() {
            return Ok(());
        }
        let _gate = self.gate.emit.lock().unwrap_or_else(|poison| poison.into_inner());
        if self.gate.epoch.load(Ordering::Acquire) != self.epoch
            || self.stream.revision.load(Ordering::Acquire) != self.revision
        {
            return Err("DMX output lifetime retired".into());
        }
        // Opening/admission may fail. Predecessor termination remains valid until a
        // replacement actually activates on a native worker, before its first packet.
        self.gate.active.store(self.epoch, Ordering::Release);
        Ok(())
    }

    pub(super) fn needs_initial_look(&self) -> bool {
        self.stream.initial_done.load(Ordering::Acquire) < self.revision
    }
    pub(super) fn finish_initial_look(&self) {
        // An old in-flight attempt cannot consume a newer authored initialization.
        self.stream.initial_done.fetch_max(self.revision, Ordering::AcqRel);
    }

    fn state<'a>(&self, state: &'a mut Option<State>, now: Instant) -> Result<&'a mut State, String> {
        self.activate()?;
        let state = state.get_or_insert_with(|| State {
            output: ClockedOutput::new(&self.config),
            confirmed: None,
            origin: now,
            epoch: self.epoch,
            revision: self.revision,
        });
        if state.epoch != self.epoch {
            let revision = self.revision;
            if state.revision != revision {
                state.confirmed = None;
                state.revision = revision;
            }
            state.output.route(&self.config);
            if let Some(frame) = &state.confirmed {
                state
                    .output
                    .restore(frame, now.saturating_duration_since(state.origin))?;
            } else {
                state.output.suspend();
            }
            state.epoch = self.epoch;
        }
        Ok(state)
    }

    fn emit(
        &self,
        bytes: &[u8],
        destination: SocketAddr,
        emit: &mut impl FnMut(&[u8], SocketAddr) -> Result<(), String>,
    ) -> Result<(), String> {
        // Different stream keys use different protocol locks. Serialize their final native
        // submissions as well, so a preempted retiring sender cannot follow its replacement.
        let _gate = self.gate.emit.lock().unwrap_or_else(|poison| poison.into_inner());
        if !self.current() {
            return Err("DMX output lifetime retired".into());
        }
        emit(bytes, destination)
    }

    pub(super) fn send(
        &self,
        frame: &DmxFrame,
        now: Instant,
        emit: &mut impl FnMut(&[u8], SocketAddr) -> Result<(), String>,
    ) -> Result<(), String> {
        self.finish_initial_look();
        let mut state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = self.state(&mut state, now)?;
        state.output.send(
            frame,
            now.saturating_duration_since(state.origin),
            &mut |bytes, destination| self.emit(bytes, destination, emit),
        )?;
        state.confirmed = Some(frame.clone());
        Ok(())
    }

    /// Also used by controlled endpoints; successful native submission alone confirms a look.
    pub(super) fn confirm(&self, frame: &DmxFrame, now: Instant) {
        let mut state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        if let Ok(state) = self.state(&mut state, now) {
            state.confirmed = Some(frame.clone());
        }
    }

    pub(super) fn command_at(&self, now: Instant) -> Instant {
        let state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        state
            .as_ref()
            .map_or(now, |state| state.origin + state.output.command_at())
    }

    pub(super) fn deadline(&self, now: Instant) -> Option<Instant> {
        if !self.current() {
            return (self.gate.epoch.load(Ordering::Acquire) == self.epoch).then_some(now);
        }
        let state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        match state.as_ref() {
            Some(state) if state.epoch == self.epoch => state.output.deadline().map(|at| state.origin + at),
            _ => Some(now),
        }
    }

    pub(super) fn poll(
        &self,
        now: Instant,
        emit: &mut impl FnMut(&[u8], SocketAddr) -> Result<(), String>,
    ) -> Result<bool, String> {
        let mut state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        let state = self.state(&mut state, now)?;
        state.output.poll(
            now.saturating_duration_since(state.origin),
            &mut |bytes, destination| self.emit(bytes, destination, emit),
        )
    }

    pub(super) fn suspend(&self) {
        let mut state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        if self.current() {
            if let Some(state) = state.as_mut() {
                state.output.suspend();
            }
        }
    }

    pub(super) fn terminate(&self) -> bool {
        let mut state = self.stream.state.lock().unwrap_or_else(|poison| poison.into_inner());
        if !self.current() {
            return false;
        }
        let Some(state) = state
            .as_mut()
            .filter(|state| state.epoch == self.epoch && state.confirmed.is_some())
        else {
            return false;
        };
        state.output.terminate();
        true
    }
}
