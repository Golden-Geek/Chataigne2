use super::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{
    engine::{EngineTime, NodeWaker},
    parameter::ParamValue,
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
    producer: Origin,
    guards: [RevisionGuard; 2],
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
enum Origin {
    Engine(NodeWaker),
    #[cfg(test)]
    Fixture,
}
impl Source {
    pub(super) fn arrive(&self) {
        self.ingress.arrive();
    }
    pub(super) fn live(&self) -> bool {
        (match &self.producer {
            Origin::Engine(producer) => !producer.is_closed(),
            #[cfg(test)]
            Origin::Fixture => true,
        }) && self.guards.iter().all(RevisionGuard::is_current)
    }
    fn weight(&self) -> usize {
        match &self.input {
            Input::Command(invocation) => (64 * 1024usize).saturating_add(invocation.override_weight.saturating_mul(3)),
            _ => 64 * 1024,
        }
    }
    pub(super) fn prepare(&self) -> Result<Request, String> {
        if !self.live() {
            return Err("Keyboard source retired; request was not sent".into());
        }
        match &self.input {
            Input::Command(invocation) => command(invocation),
            Input::Script { method, args } => {
                if args.len() > 1 || args.retained_bytes() > 64 * 1024 {
                    return Err("Keyboard scripts accept one argument / 64 KiB".into());
                }
                let key = parse_key(args.first())?;
                let action = match *method {
                    "tapKey" => KeyboardKeyAction::Tap,
                    "pressKey" => KeyboardKeyAction::Press,
                    "releaseKey" => KeyboardKeyAction::Release,
                    _ => return Err("Unknown Keyboard script method".into()),
                };
                Ok(Request { key, action })
            }
            #[cfg(test)]
            Input::Prepared(request) => Ok(*request),
        }
    }
}
fn text(value: &ParamValue) -> Result<&str, String> {
    let text = match value {
        ParamValue::Str(text) | ParamValue::Enum(text) => text.as_str(),
        _ => return Err("Keyboard key/action must be a string or enum".into()),
    };
    if text.len() > 64 {
        return Err("Keyboard key/action text exceeds 64 bytes".into());
    }
    Ok(text)
}
fn parse_key(value: Option<&ParamValue>) -> Result<KeyboardKey, String> {
    KeyboardKey::parse(text(value.ok_or("Keyboard method requires a key id string")?)?)
}
fn command(invocation: &CapturedInvocation) -> Result<Request, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|event| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**event).clone()).map_err(|error| error.to_string())
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("Keyboard invocation targets another command".into());
    }
    let field = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|event| event.param_overrides.iter().rev().find(|value| value.param_id == id))
            .map(|value| &value.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    let key = field("key")
        .map(|value| KeyboardKey::parse(text(value)?))
        .transpose()?
        .unwrap_or(KeyboardKey::Space);
    let action = field("action")
        .map(|value| KeyboardKeyAction::parse(text(value)?))
        .transpose()?
        .unwrap_or(KeyboardKeyAction::Tap);
    Ok(Request { key, action })
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
            return Err("Keyboard output worker is stopped; request was not admitted".into());
        }
        let actor = self.actor.as_ref().ok_or("Keyboard output is stopped")?;
        let source = Source {
            input,
            time,
            producer,
            guards: [actor.reader().guard_at(0), self.shared.native.guard_at(0)],
            _admission: permit,
            ingress: Ingress {
                permit: Mutex::new(Some(ingress)),
                notifier: self.shared.notifier.clone(),
            },
        };
        let weight = source.weight();
        if weight > 4 * 1024 * 1024 {
            return Err("Keyboard source exceeds 4 MiB mailbox capacity".into());
        }
        self.pool
            .try_retain(source, weight)
            .map_err(|_| "Keyboard source lifetime capacity is full".into())
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
