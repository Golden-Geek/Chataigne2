use std::f64::consts::PI;

use golden_core::node::{Curve, NodeId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct SignalSignature {
    pub(super) shape: SignalShape,
    pub(super) frequency_hz: u64,
    pub(super) phase: u64,
    pub(super) seed: i32,
}

#[derive(Clone, Debug)]
pub(super) struct SignalRuntimeState {
    pub(super) elapsed_seconds: f64,
    pub(super) frequency_offset_cycles: f64,
    pub(super) last_cycle: i64,
    pub(super) pure_random_step: i64,
    pub(super) pure_random_value: f64,
    pub(super) signature: Option<SignalSignature>,
}

impl Default for SignalRuntimeState {
    fn default() -> Self {
        Self {
            elapsed_seconds: 0.0,
            frequency_offset_cycles: 0.0,
            last_cycle: 0,
            pure_random_step: i64::MIN,
            pure_random_value: 0.0,
            signature: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SignalShape {
    Sine,
    Triangle,
    Saw,
    ReverseSaw,
    RandomPerlin,
    RandomPure,
    Curve,
}

#[derive(Clone, Debug)]
pub(super) struct SignalConfig {
    pub(super) item_id: NodeId,
    pub(super) value_decl_id: String,
    pub(super) label: String,
    pub(super) enabled: bool,
    pub(super) shape: SignalShape,
    pub(super) frequency_hz: f64,
    pub(super) phase: f64,
    pub(super) range_min: f64,
    pub(super) range_max: f64,
    pub(super) curve: Option<Curve>,
    pub(super) seed: i32,
}

impl SignalConfig {
    fn signature(&self) -> SignalSignature {
        SignalSignature {
            shape: self.shape,
            frequency_hz: self.frequency_hz.to_bits(),
            phase: self.phase.to_bits(),
            seed: self.seed,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct SignalSample {
    pub(super) value: f64,
    pub(super) cycle: i64,
}

pub(super) fn sample_signal(config: &SignalConfig, state: &mut SignalRuntimeState) -> SignalSample {
    let phase_position = config.phase + state.frequency_offset_cycles + state.elapsed_seconds * config.frequency_hz;
    let phase = normalized_phase(phase_position);
    let cycle = phase_position.floor() as i64;
    let unit = match config.shape {
        SignalShape::Sine => 0.5 + 0.5 * (phase * PI * 2.0).sin(),
        SignalShape::Triangle => 1.0 - (2.0 * phase - 1.0).abs(),
        SignalShape::Saw => phase,
        SignalShape::ReverseSaw => 1.0 - phase,
        SignalShape::RandomPerlin => smooth_noise(config.seed as i64, phase_position),
        SignalShape::RandomPure => pure_random_unit(config, state, cycle),
        SignalShape::Curve => curve_unit(config, phase),
    }
    .clamp(0.0, 1.0);

    SignalSample {
        value: config.range_min + (config.range_max - config.range_min) * unit,
        cycle,
    }
}

pub(super) fn synchronize_signal_state(state: &mut SignalRuntimeState, config: &SignalConfig) {
    let signature = config.signature();
    if state.signature == Some(signature) {
        return;
    }
    if let Some(previous) = state.signature {
        // Tempo edits change the future rate, preserving the accumulated phase.
        state.frequency_offset_cycles +=
            state.elapsed_seconds * (f64::from_bits(previous.frequency_hz) - config.frequency_hz);
    } else {
        state.last_cycle = config.phase.floor() as i64;
    }
    state.pure_random_step = i64::MIN;
    state.pure_random_value = 0.0;
    state.signature = Some(signature);
}

fn curve_unit(config: &SignalConfig, phase: f64) -> f64 {
    config
        .curve
        .as_ref()
        .and_then(|curve| curve.sample(phase))
        .unwrap_or(0.0)
}

fn pure_random_unit(config: &SignalConfig, state: &mut SignalRuntimeState, step: i64) -> f64 {
    if state.pure_random_step != step {
        state.pure_random_step = step;
        state.pure_random_value = hash_unit(config.seed as i64, step);
    }
    state.pure_random_value
}

fn smooth_noise(seed: i64, position: f64) -> f64 {
    let left = position.floor();
    let right = left + 1.0;
    let t = smooth_step(position - left);
    let a = hash_unit(seed, left as i64);
    let b = hash_unit(seed, right as i64);
    a + (b - a) * t
}

fn smooth_step(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn normalized_phase(value: f64) -> f64 {
    let phase = value.fract();
    if phase.is_sign_negative() {
        phase + 1.0
    } else {
        phase
    }
}

fn hash_unit(seed: i64, index: i64) -> f64 {
    let mut x = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ index as u64;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}
