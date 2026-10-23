use super::coordination::Origin;
use super::*;
use crate::app::module_command::CapturedInvocation;
use golden_core::{
    engine::{EngineTime, NodeWaker},
    script::ScriptArguments,
};
use golden_io::{PendingSendError, RetirementPermit};
use std::sync::Mutex;

pub(super) enum Input {
    Command(CapturedInvocation),
    Script {
        method: &'static str,
        args: ScriptArguments,
    },
    #[cfg(test)]
    Prepared(Request),
}
pub(super) struct Source {
    input: Input,
    pub(super) time: EngineTime,
    pub(super) ticket: coordination::Ticket,
    _admission: RetirementPermit,
    ingress: Ingress,
}
struct Ingress {
    permit: Mutex<Option<RetirementPermit>>,
    notifier: WorkerNotifier,
}
impl Ingress {
    fn arrive(&self) {
        if self
            .permit
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .is_some()
        {
            self.notifier.notify();
        }
    }
    fn pending(&self) -> bool {
        self.permit.lock().unwrap_or_else(|error| error.into_inner()).is_some()
    }
}
impl Drop for Ingress {
    fn drop(&mut self) {
        self.arrive();
    }
}
impl Source {
    pub(super) fn arrive(&self) {
        self.ingress.arrive();
    }
    pub(super) fn live(&self) -> bool {
        self.ticket.live()
    }
    fn weight(&self) -> usize {
        match &self.input {
            Input::Command(invocation) => (64 * 1024usize).saturating_add(invocation.override_weight.saturating_mul(3)),
            _ => 64 * 1024,
        }
    }
    pub(super) fn prepare(&self) -> Result<Request, String> {
        if !self.live() {
            return Err("Mouse source retired; request was not sent".into());
        }
        match &self.input {
            Input::Command(invocation) => preparation::command(invocation),
            Input::Script { method, args } => preparation::script(method, args),
            #[cfg(test)]
            Input::Prepared(request) => Ok(*request),
        }
    }
}

impl Runtime {
    pub(super) fn capture(
        &self,
        input: Input,
        producer: NodeWaker,
        time: EngineTime,
        permit: RetirementPermit,
        ingress: RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        self.capture_inner(input, Origin::Engine(producer), time, permit, ingress)
    }
    #[cfg(test)]
    pub(super) fn capture_fixture(
        &self,
        input: Input,
        time: EngineTime,
        permit: RetirementPermit,
        ingress: RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        self.capture_inner(input, Origin::Fixture, time, permit, ingress)
    }
    fn capture_inner(
        &self,
        input: Input,
        producer: Origin,
        time: EngineTime,
        permit: RetirementPermit,
        ingress: RetirementPermit,
    ) -> Result<RetainedPayload<Source>, String> {
        if !self.is_running() {
            return Err("Mouse output worker is stopped; request was not admitted".into());
        }
        let actor = self.actor.as_ref().ok_or("Mouse output is stopped")?;
        let source = Source {
            input,
            time,
            ticket: self.coordinator.capture(
                time,
                producer,
                [actor.reader().guard_at(0), self.shared.native.guard_at(0)],
            )?,
            _admission: permit,
            ingress: Ingress {
                permit: Mutex::new(Some(ingress)),
                notifier: self.shared.notifier.clone(),
            },
        };
        let weight = source.weight();
        if weight > 4 * 1024 * 1024 {
            return Err("Mouse source exceeds 4 MiB mailbox capacity".into());
        }
        self.pool
            .try_retain(source, weight)
            .map_err(|_| "Mouse source lifetime capacity is full".into())
    }
    pub(super) fn submit(&self, source: RetainedPayload<Source>) -> Result<Receipt, RetainedPayload<Source>> {
        if source.ingress.pending() {
            return Err(source);
        }
        let weight = source.weight();
        let (command, receipt) = Command::new(source.clone(), self.shared.notifier.clone());
        match self.worker.send_weighted(command, weight) {
            Ok(()) => Ok(receipt),
            Err(PendingSendError::Full(mut command) | PendingSendError::Disconnected(mut command)) => {
                command.armed = false;
                Err(source)
            }
        }
    }
}
