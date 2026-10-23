use super::*;
use crate::app::module::common::joycon::*;
use crate::app::module_command::{CapturedInvocation, ModuleCommandExecuteEvent};
use golden_core::{
    engine::{EngineTime, NodeWaker},
    parameter::ParamValue,
    script::ScriptArguments,
};
use golden_io::RetirementPermit;

pub(in super::super) enum Input {
    Command(CapturedInvocation),
    Script {
        method: &'static str,
        args: ScriptArguments,
    },
    #[cfg(test)]
    Prepared(JoyConWorkerCommand),
}
pub(in super::super) struct Source {
    pub(in super::super) input: Input,
    pub(in super::super) time: EngineTime,
    pub(super) origin: Origin,
    pub(super) guards: [RevisionGuard; 3],
    pub(super) _admission: RetirementPermit,
    pub(super) ingress: Ingress,
}
pub(super) struct Ingress {
    permit: Mutex<Option<RetirementPermit>>,
    notifier: WorkerNotifier,
}
impl Ingress {
    pub(super) fn new(permit: RetirementPermit, notifier: WorkerNotifier) -> Self {
        Self {
            permit: Mutex::new(Some(permit)),
            notifier,
        }
    }
    #[cfg(test)]
    pub(super) fn ready() -> Self {
        Self {
            permit: Mutex::new(None),
            notifier: WorkerNotifier::new(|| {}),
        }
    }
    fn arrive(&self) {
        let permit = self.permit.lock().unwrap_or_else(|error| error.into_inner()).take();
        if permit.is_some() {
            drop(permit);
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
pub(super) enum Origin {
    Engine(NodeWaker),
    #[cfg(test)]
    Fixture,
}
impl Source {
    pub(in super::super) fn arrive(&self) {
        self.ingress.arrive();
    }
    pub(super) fn pending_ingress(&self) -> bool {
        self.ingress.pending()
    }
    pub(in super::super) fn live(&self) -> bool {
        let producer_live = match &self.origin {
            Origin::Engine(source) => !source.is_closed(),
            #[cfg(test)]
            Origin::Fixture => true,
        };
        producer_live && self.guards.iter().all(RevisionGuard::is_current)
    }
    pub(in super::super) fn weight(&self) -> usize {
        match &self.input {
            Input::Command(invocation) => (64 * 1024usize).saturating_add(invocation.override_weight.saturating_mul(3)),
            Input::Script { .. } => 64 * 1024,
            #[cfg(test)]
            Input::Prepared(_) => COMMAND_WEIGHT,
        }
    }
    pub(super) fn prepare(&self) -> Result<JoyConWorkerCommand, String> {
        if !self.live() {
            return Err("Joy-Con source retired; request was not sent".into());
        }
        let request = match &self.input {
            Input::Command(invocation) => command(invocation)?,
            Input::Script { method, args } => {
                if args.len() > 5 || args.retained_bytes() > 64 * 1024 {
                    return Err("Joy-Con scripts accept five arguments / 64 KiB".into());
                }
                match *method {
                    "vibrate" => vibrate(|index| args.get(index))?,
                    "setPlayerLights" => lights(|index| args.get(index))?,
                    _ => return Err("Unknown Joy-Con method".into()),
                }
            }
            #[cfg(test)]
            Input::Prepared(request) => request.clone(),
        };
        validate_command(&request)?;
        Ok(request)
    }
}
fn command(invocation: &CapturedInvocation) -> Result<JoyConWorkerCommand, String> {
    let execution = invocation
        .execution
        .as_ref()
        .map(|value| {
            serde_json::from_value::<ModuleCommandExecuteEvent>((**value).clone()).map_err(|error| error.to_string())
        })
        .transpose()?;
    if execution
        .as_ref()
        .is_some_and(|event| event.command_id != invocation.command)
    {
        return Err("Joy-Con invocation targets another command".into());
    }
    let value = |name| {
        let id = invocation.snapshot.find_child_by_decl_id(invocation.command, name)?;
        execution
            .as_ref()
            .and_then(|event| event.param_overrides.iter().rev().find(|entry| entry.param_id == id))
            .map(|entry| &entry.value)
            .or_else(|| invocation.snapshot.node(id)?.param_value.as_ref())
    };
    match invocation
        .snapshot
        .node(invocation.command)
        .map(|node| node.node_type.as_str())
    {
        Some(JOYCON_VIBRATE_COMMAND_NODE_TYPE) => {
            vibrate(|index| value(["frequency_hz", "amplitude", "duration_ms", "target"][index]))
        }
        Some(JOYCON_SET_LED_COMMAND_NODE_TYPE) => {
            lights(|index| value(["led_1", "led_2", "led_3", "led_4", "target"][index]))
        }
        _ => Err("Unsupported Joy-Con command type".into()),
    }
}
fn number(value: Option<&ParamValue>, default: f64) -> Result<f64, String> {
    let Some(value) = value else {
        return Ok(default);
    };
    if matches!(value, ParamValue::Str(text) | ParamValue::Enum(text) | ParamValue::File(text) if text.len() > 32) {
        return Err("Joy-Con numeric text exceeds 32 bytes".into());
    }
    let number = value
        .as_float()
        .or_else(|| value.as_int().map(f64::from))
        .ok_or("Joy-Con value must be numeric")?;
    if !number.is_finite() {
        return Err("Joy-Con numeric values must be finite".into());
    }
    Ok(number)
}
fn text<'a>(value: Option<&'a ParamValue>, default: &'a str) -> Result<&'a str, String> {
    match value {
        None => Ok(default),
        Some(ParamValue::Str(text) | ParamValue::Enum(text)) if text.len() <= 256 => Ok(text.trim()),
        _ => Err("Joy-Con text must be a string within 256 bytes".into()),
    }
}
fn target(value: Option<&ParamValue>) -> Result<String, String> {
    JoyConControllerTarget::from_variant_id(text(value, JOYCON_TARGET_BOTH)?)
        .map(|target| target.variant_id().to_owned())
        .ok_or("Joy-Con target must be left, right or both".into())
}
fn vibrate<'a>(value: impl Fn(usize) -> Option<&'a ParamValue>) -> Result<JoyConWorkerCommand, String> {
    let frequency_hz = number(value(0), 300.0)?;
    let amplitude = number(value(1), 0.9)?;
    let duration = number(value(2), 60.0)?.round();
    if !(0.0..=1252.0).contains(&frequency_hz) || !(0.0..=1.799).contains(&amplitude) {
        return Err("Joy-Con frequency/amplitude is outside its supported range".into());
    }
    if !(1.0..=f64::from(u32::MAX)).contains(&duration) {
        return Err("Joy-Con duration must be a positive u32 millisecond count".into());
    }
    Ok(JoyConWorkerCommand::Vibrate(JoyConVibrateRequest {
        target: target(value(3))?,
        frequency_hz: frequency_hz as f32,
        amplitude: amplitude as f32,
        duration_ms: duration as u32,
        description: "vibrate".into(),
    }))
}
fn lights<'a>(value: impl Fn(usize) -> Option<&'a ParamValue>) -> Result<JoyConWorkerCommand, String> {
    let led = |index| match text(value(index), JOYCON_LED_STATE_OFF)? {
        JOYCON_LED_STATE_OFF => Ok(JOYCON_LED_STATE_OFF.to_owned()),
        JOYCON_LED_STATE_ON => Ok(JOYCON_LED_STATE_ON.to_owned()),
        JOYCON_LED_STATE_FLASH => Ok(JOYCON_LED_STATE_FLASH.to_owned()),
        _ => Err("Joy-Con LED state must be off, on or flash".to_owned()),
    };
    Ok(JoyConWorkerCommand::SetPlayerLights(JoyConSetLedRequest {
        target: target(value(4))?,
        led_1: led(0)?,
        led_2: led(1)?,
        led_3: led(2)?,
        led_4: led(3)?,
        description: "set player lights".into(),
    }))
}
