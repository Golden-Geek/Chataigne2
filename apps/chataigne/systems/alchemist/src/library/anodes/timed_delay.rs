use crate::{ANodeInstance, CompiledNodeEvaluator, NodeEvaluation, RuntimeValue};

use super::support::{config_float, config_int};

const MAX_QUEUE_ITEMS: usize = 128;
const MAX_QUEUE_BYTES: usize = 1024 * 1024;
const MAX_VALUE_BYTES: usize = 64 * 1024;

#[derive(Debug)]
pub(super) struct TimedDelayEval {
    seconds: f64,
    capacity: usize,
}

impl TimedDelayEval {
    pub(super) fn from_config(instance: &ANodeInstance) -> Result<Self, String> {
        let seconds = config_float(instance, "seconds", 0.1);
        if !seconds.is_finite() || !(0.0..=3600.0).contains(&seconds) {
            return Err("Timed Delay requires a finite duration from 0 to 3600 seconds".into());
        }
        let capacity = config_int(instance, "capacity", 64);
        if !(1..=MAX_QUEUE_ITEMS as i64).contains(&capacity) {
            return Err("Timed Delay capacity must be from 1 to 128 items".into());
        }
        Ok(Self {
            seconds,
            capacity: capacity as usize,
        })
    }
}

impl CompiledNodeEvaluator for TimedDelayEval {
    fn evaluate(&self, evaluation: &mut NodeEvaluation<'_, '_>) -> Result<crate::NodeOutputs, String> {
        let [value] = evaluation.inputs else {
            return Err("Timed Delay expects one input".into());
        };
        if self.seconds == 0.0 {
            evaluation.state.fill(RuntimeValue::Unit);
            return Ok(crate::node_outputs![value.clone()]);
        }
        let size = value_size(value);
        if size > MAX_VALUE_BYTES {
            evaluation.state.fill(RuntimeValue::Unit);
            return Err("Timed Delay input exceeds the 64 KiB value limit".into());
        }
        let previous_clock = match evaluation.state.first() {
            Some(RuntimeValue::Float(clock)) if clock.is_finite() && *clock >= 0.0 => *clock,
            _ => 0.0,
        };
        let clock = previous_clock + evaluation.ctx.delta_time.as_secs_f64();
        if !clock.is_finite() {
            evaluation.state.fill(RuntimeValue::Unit);
            return Err("Timed Delay clock overflow".into());
        }
        let mut pending = match std::mem::replace(&mut evaluation.state[1], RuntimeValue::Unit) {
            RuntimeValue::Array(entries) => entries,
            _ => Vec::new(),
        };
        let ready = pending.first().and_then(|entry| match entry {
            RuntimeValue::Array(parts) => match parts.as_slice() {
                [RuntimeValue::Float(due), _] if *due <= clock => Some(()),
                _ => None,
            },
            _ => None,
        });
        let output = if ready.is_some() {
            match pending.remove(0) {
                RuntimeValue::Array(mut parts) => parts.pop(),
                _ => None,
            }
        } else {
            None
        };
        if pending.len() >= self.capacity {
            evaluation.state.fill(RuntimeValue::Unit);
            return Err("Timed Delay queue capacity exceeded; pending values were reset".into());
        }
        let pending_bytes = pending.iter().map(value_size).sum::<usize>();
        if pending_bytes.saturating_add(size).saturating_add(64) > MAX_QUEUE_BYTES {
            evaluation.state.fill(RuntimeValue::Unit);
            return Err("Timed Delay queue exceeds the 1 MiB memory limit; pending values were reset".into());
        }
        let due = clock + self.seconds;
        if !due.is_finite() {
            evaluation.state.fill(RuntimeValue::Unit);
            return Err("Timed Delay due time overflow".into());
        }
        pending.push(RuntimeValue::Array(vec![RuntimeValue::Float(due), value.clone()]));
        evaluation.state[0] = RuntimeValue::Float(clock);
        evaluation.state[1] = RuntimeValue::Array(pending);
        if let Some(output) = output {
            Ok(crate::node_outputs![output])
        } else {
            evaluation.suppress_output(0);
            Ok(crate::node_outputs![RuntimeValue::Unit])
        }
    }
}

fn value_size(value: &RuntimeValue) -> usize {
    let mut size = 0usize;
    let mut stack = vec![value];
    while let Some(value) = stack.pop() {
        size = size.saturating_add(std::mem::size_of::<RuntimeValue>());
        match value {
            RuntimeValue::String(value) => size = size.saturating_add(value.len()),
            RuntimeValue::Ref(value) => {
                size = size.saturating_add(value.stable_id.len() + value.value_type.as_str().len());
            }
            RuntimeValue::Extension(value) => size = size.saturating_add(value.payload.len()),
            RuntimeValue::Array(values) => stack.extend(values),
            _ => {}
        }
        if size > MAX_QUEUE_BYTES {
            return size;
        }
    }
    size
}
