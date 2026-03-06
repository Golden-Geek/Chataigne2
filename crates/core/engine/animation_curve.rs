use std::cell::OnceCell;
use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

const CURVE_EPSILON: f64 = 1e-12;
const MAX_PERLIN_OCTAVES: u32 = 12;

/// Handle coordinates used by bezier easing.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CurveHandle {
    /// Handle position coordinate.
    pub position: f64,
    /// Handle value coordinate.
    pub value: f64,
}

impl CurveHandle {
    /// Creates one handle from `position` and `value`.
    pub fn new(position: f64, value: f64) -> Self {
        Self { position, value }
    }
}

impl Default for CurveHandle {
    fn default() -> Self {
        Self { position: 0.0, value: 0.0 }
    }
}

/// Step quantization strategy for [`CurveEasing::Steps`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CurveStepMode {
    /// Step count is derived from one step size expressed on the segment position axis.
    StepSize,
    /// Step count is directly specified.
    #[default]
    NumSteps,
}

/// Shape waveform used by [`CurveEasing::Shape`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CurveShape {
    /// Smooth sinus wave.
    #[default]
    Sine,
    /// Triangle wave in range `[-1, 1]`.
    Triangle,
    /// Rising saw wave in range `[-1, 1]`.
    Saw,
    /// Falling saw wave in range `[-1, 1]`.
    ReverseSaw,
    /// Bipolar square wave in range `[-1, 1]`.
    Square,
}

/// Phase-control mode for [`CurveEasing::Shape`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum CurvePhaseMode {
    /// `frequency` is interpreted as cycles per position unit.
    #[default]
    Frequency,
    /// `num_phases` is interpreted as total cycle count over the segment.
    NumPhases,
}

/// Easing attached to one key and used for interpolation toward the next key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CurveEasing {
    /// Standard linear interpolation.
    #[default]
    Linear,
    /// Cubic bezier interpolation.
    Bezier {
        /// Outgoing handle anchored at the current key.
        #[serde(default = "default_bezier_out_handle")]
        out_handle: CurveHandle,
        /// Incoming handle anchored at the next key.
        #[serde(default = "default_bezier_in_handle")]
        in_handle: CurveHandle,
    },
    /// Hold value until the next key boundary.
    Hold,
    /// Quantized stepped interpolation.
    Steps {
        /// Strategy used to resolve effective step count.
        #[serde(default)]
        step_mode: CurveStepMode,
        /// Step size used when `step_mode = stepSize`.
        #[serde(default = "default_step_size")]
        step_size: f64,
        /// Explicit step count used when `step_mode = numSteps`.
        #[serde(default = "default_num_steps")]
        num_steps: u32,
    },
    /// Shape modulation over the base linear segment.
    Shape {
        /// Shape waveform.
        #[serde(default)]
        shape: CurveShape,
        /// Modulation amplitude.
        #[serde(default = "default_shape_amplitude")]
        amplitude: f64,
        /// How to interpret phase controls.
        #[serde(default)]
        phase_mode: CurvePhaseMode,
        /// Cycles per position unit (used when `phase_mode = frequency`).
        #[serde(default = "default_shape_frequency")]
        frequency: f64,
        /// Total cycle count over the segment (used when `phase_mode = numPhases`).
        #[serde(default = "default_shape_num_phases")]
        num_phases: f64,
        /// Relative fade-in span in `[0, +inf)`.
        #[serde(default)]
        fade_in: f64,
        /// Relative fade-out span in `[0, +inf)`.
        #[serde(default)]
        fade_out: f64,
    },
    /// Fractal value-noise modulation over the base linear segment.
    PerlinNoise {
        /// Base frequency (cycles per position unit).
        #[serde(default = "default_noise_frequency")]
        frequency: f64,
        /// Noise amplitude.
        #[serde(default = "default_noise_amplitude")]
        amplitude: f64,
        /// Number of octaves used by fractal noise.
        #[serde(default = "default_noise_octaves")]
        octaves: u32,
        /// Relative fade-in span in `[0, +inf)`.
        #[serde(default)]
        fade_in: f64,
        /// Relative fade-out span in `[0, +inf)`.
        #[serde(default)]
        fade_out: f64,
        /// Additional phase offset applied to the noise domain.
        #[serde(default)]
        phase: f64,
    },
    /// Seeded random interpolation modulation.
    Random {
        /// Bucket frequency (changes per position unit).
        #[serde(default = "default_random_frequency")]
        frequency: f64,
        /// Relative fade-in span in `[0, +inf)`.
        #[serde(default)]
        fade_in: f64,
        /// Relative fade-out span in `[0, +inf)`.
        #[serde(default)]
        fade_out: f64,
        /// Deterministic random seed.
        #[serde(default)]
        seed: u64,
    },
    /// Script-defined easing.
    Script {
        /// Script source or script identifier resolved by the host.
        source: String,
    },
}

fn default_bezier_out_handle() -> CurveHandle {
    CurveHandle::new(1.0 / 3.0, 1.0 / 3.0)
}

fn default_bezier_in_handle() -> CurveHandle {
    CurveHandle::new(-1.0 / 3.0, -1.0 / 3.0)
}

fn default_step_size() -> f64 {
    0.1
}

fn default_num_steps() -> u32 {
    8
}

fn default_shape_amplitude() -> f64 {
    1.0
}

fn default_shape_frequency() -> f64 {
    1.0
}

fn default_shape_num_phases() -> f64 {
    1.0
}

fn default_noise_frequency() -> f64 {
    1.0
}

fn default_noise_amplitude() -> f64 {
    1.0
}

fn default_noise_octaves() -> u32 {
    4
}

fn default_random_frequency() -> f64 {
    6.0
}

/// One key of an animation curve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AnimationCurveKey {
    /// Key position on the curve domain axis.
    pub position: f64,
    /// Key value on the curve value axis.
    pub value: f64,
    /// Easing applied from this key to the next key.
    #[serde(default)]
    pub easing: CurveEasing,
}

impl AnimationCurveKey {
    /// Creates one key with `position`, `value`, and `easing`.
    pub fn new(position: f64, value: f64, easing: CurveEasing) -> Self {
        Self { position, value, easing }
    }
}

/// Runtime context passed to script-based easing callbacks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CurveSampleContext {
    /// Current sampled position.
    pub position: f64,
    /// Segment-relative progress in `[0, 1]`.
    pub progress: f64,
    /// Start position of the segment being sampled.
    pub segment_start_position: f64,
    /// End position of the segment being sampled.
    pub segment_end_position: f64,
    /// Start value of the segment being sampled.
    pub segment_start_value: f64,
    /// End value of the segment being sampled.
    pub segment_end_value: f64,
    /// Linear interpolation value at the current `progress`.
    pub linear_value: f64,
}

/// Callback interface used by [`CurveEasing::Script`].
pub trait CurveScriptSampler {
    /// Evaluates one script easing sample.
    ///
    /// Returning `None` falls back to linear interpolation for that sample.
    fn sample(&mut self, source: &str, context: CurveSampleContext) -> Option<f64>;
}

/// Stateful cursor used to accelerate sequential sampling.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimationCurveCursor {
    index: usize,
    initialized: bool,
}

impl AnimationCurveCursor {
    /// Creates a cursor with no cached segment.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears cached segment state.
    pub fn reset(&mut self) {
        self.index = 0;
        self.initialized = false;
    }
}

/// Sorted key/easing animation curve with compiled segment cache.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnimationCurve {
    /// Sorted curve keys.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    keys: Vec<AnimationCurveKey>,
    /// Optional sampled-value clamp range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value_range_constraint: Option<(f64, f64)>,
    #[serde(skip, default)]
    compiled_segments: OnceCell<Vec<CompiledCurveSegment>>,
}

impl AnimationCurve {
    /// Builds one curve from arbitrary keys.
    ///
    /// Keys are normalized during construction:
    /// - non-finite keys are dropped
    /// - keys are sorted by position
    /// - duplicate positions keep the last key
    pub fn new(mut keys: Vec<AnimationCurveKey>) -> Self {
        normalize_keys(&mut keys);
        Self {
            keys,
            value_range_constraint: None,
            compiled_segments: OnceCell::new(),
        }
    }

    /// Returns one copy of the sampled-value clamp range.
    pub fn value_range_constraint(&self) -> Option<(f64, f64)> {
        self.value_range_constraint
    }

    /// Sets/clears sampled-value clamping.
    pub fn set_value_range_constraint(&mut self, min: Option<f64>, max: Option<f64>) {
        self.value_range_constraint = normalize_value_range(min, max);
    }

    /// Fluent wrapper around [`Self::set_value_range_constraint`].
    pub fn with_value_range_constraint(mut self, min: Option<f64>, max: Option<f64>) -> Self {
        self.set_value_range_constraint(min, max);
        self
    }

    /// Returns immutable keys.
    pub fn keys(&self) -> &[AnimationCurveKey] {
        self.keys.as_slice()
    }

    /// Returns key count.
    pub fn key_count(&self) -> usize {
        self.keys.len()
    }

    /// Returns compiled segment count.
    pub fn segment_count(&self) -> usize {
        self.compiled_segments().len()
    }

    /// Replaces all keys.
    pub fn set_keys(&mut self, mut keys: Vec<AnimationCurveKey>) {
        normalize_keys(&mut keys);
        self.keys = keys;
        self.compiled_segments = OnceCell::new();
    }

    /// Inserts one key while preserving sort order.
    ///
    /// If another key already exists at the same position, it is replaced.
    /// Returns the resulting key index.
    pub fn insert_key(&mut self, key: AnimationCurveKey) -> usize {
        if !key.position.is_finite() || !key.value.is_finite() {
            return self.keys.len();
        }

        let index = self.keys.partition_point(|existing| existing.position < key.position);
        if let Some(existing) = self.keys.get(index) {
            if (existing.position - key.position).abs() <= CURVE_EPSILON {
                self.keys[index] = key;
                self.compiled_segments = OnceCell::new();
                return index;
            }
        }

        self.keys.insert(index, key);
        self.compiled_segments = OnceCell::new();
        index
    }

    /// Removes one key by index.
    pub fn remove_key(&mut self, index: usize) -> Option<AnimationCurveKey> {
        if index >= self.keys.len() {
            return None;
        }

        self.compiled_segments = OnceCell::new();
        Some(self.keys.remove(index))
    }

    /// Samples one value at `position`.
    pub fn sample(&self, position: f64) -> Option<f64> {
        let mut script_sampler = None;
        self.sample_internal(position, None, &mut script_sampler)
    }

    /// Samples one value at `position` using a sequential cursor.
    pub fn sample_with_cursor(&self, position: f64, cursor: &mut AnimationCurveCursor) -> Option<f64> {
        let mut script_sampler = None;
        self.sample_internal(position, Some(cursor), &mut script_sampler)
    }

    /// Samples one value at `position` and resolves script easings with `script_sampler`.
    pub fn sample_with_script(&self, position: f64, script_sampler: &mut dyn CurveScriptSampler) -> Option<f64> {
        let mut script_sampler = Some(script_sampler);
        self.sample_internal(position, None, &mut script_sampler)
    }

    /// Samples one value at `position` with cursor acceleration and script easing support.
    pub fn sample_with_cursor_and_script(&self, position: f64, cursor: &mut AnimationCurveCursor, script_sampler: &mut dyn CurveScriptSampler) -> Option<f64> {
        let mut script_sampler = Some(script_sampler);
        self.sample_internal(position, Some(cursor), &mut script_sampler)
    }

    /// Samples uniformly between `start_position` and `end_position` into `output`.
    ///
    /// Returns the number of written samples.
    pub fn sample_range(&self, start_position: f64, end_position: f64, output: &mut [f64]) -> usize {
        let mut script_sampler = None;
        self.sample_range_internal(start_position, end_position, output, &mut script_sampler)
    }

    /// Samples uniformly between `start_position` and `end_position` into `output`.
    ///
    /// Script easings are resolved with `script_sampler`.
    /// Returns the number of written samples.
    pub fn sample_range_with_script(&self, start_position: f64, end_position: f64, output: &mut [f64], script_sampler: &mut dyn CurveScriptSampler) -> usize {
        let mut script_sampler = Some(script_sampler);
        self.sample_range_internal(start_position, end_position, output, &mut script_sampler)
    }

    fn sample_range_internal(&self, start_position: f64, end_position: f64, output: &mut [f64], script_sampler: &mut Option<&mut dyn CurveScriptSampler>) -> usize {
        if output.is_empty() || !start_position.is_finite() || !end_position.is_finite() {
            return 0;
        }

        if self.keys.is_empty() {
            return 0;
        }

        if self.keys.len() == 1 {
            output.fill(self.clamp_sampled_value(self.keys[0].value));
            return output.len();
        }

        if output.len() == 1 {
            if let Some(value) = self.sample_internal(start_position, None, script_sampler) {
                output[0] = value;
                return 1;
            }
            return 0;
        }

        let output_len = output.len();
        let step = (end_position - start_position) / (output_len.saturating_sub(1) as f64);
        let mut cursor = AnimationCurveCursor::new();
        for (index, slot) in output.iter_mut().enumerate() {
            let position = if index + 1 == output_len { end_position } else { start_position + step * (index as f64) };
            if let Some(sampled) = self.sample_internal(position, Some(&mut cursor), script_sampler) {
                *slot = sampled;
            } else {
                return index;
            }
        }

        output.len()
    }

    fn sample_internal(&self, position: f64, cursor: Option<&mut AnimationCurveCursor>, script_sampler: &mut Option<&mut dyn CurveScriptSampler>) -> Option<f64> {
        if !position.is_finite() || self.keys.is_empty() {
            return None;
        }

        if self.keys.len() == 1 {
            return Some(self.clamp_sampled_value(self.keys[0].value));
        }

        let first = self.keys.first()?;
        let last = self.keys.last()?;
        if position <= first.position {
            return Some(self.clamp_sampled_value(first.value));
        }
        if position >= last.position {
            return Some(self.clamp_sampled_value(last.value));
        }

        let segments = self.compiled_segments();
        if segments.is_empty() {
            return Some(self.clamp_sampled_value(first.value));
        }

        let segment_index = if let Some(cursor) = cursor { resolve_segment_index_with_cursor(position, segments, cursor) } else { resolve_segment_index(position, segments) };

        let segment = segments.get(segment_index)?;
        Some(self.clamp_sampled_value(sample_compiled_segment(segment, position, script_sampler)))
    }

    fn compiled_segments(&self) -> &[CompiledCurveSegment] {
        self.compiled_segments.get_or_init(|| compile_segments(self.keys.as_slice())).as_slice()
    }

    fn clamp_sampled_value(&self, value: f64) -> f64 {
        if let Some((min, max)) = self.value_range_constraint {
            return value.max(min).min(max);
        }
        value
    }
}

impl Default for AnimationCurve {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl PartialEq for AnimationCurve {
    fn eq(&self, other: &Self) -> bool {
        self.keys == other.keys && self.value_range_constraint == other.value_range_constraint
    }
}

fn normalize_value_range(min: Option<f64>, max: Option<f64>) -> Option<(f64, f64)> {
    let (Some(mut min), Some(mut max)) = (min, max) else {
        return None;
    };
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    if min > max {
        std::mem::swap(&mut min, &mut max);
    }
    if (max - min).abs() <= CURVE_EPSILON {
        return None;
    }
    Some((min, max))
}

#[derive(Clone, Debug)]
struct CompiledCurveSegment {
    start_position: f64,
    end_position: f64,
    start_value: f64,
    end_value: f64,
    value_delta: f64,
    inv_span: f64,
    easing: CompiledCurveEasing,
}

#[derive(Clone, Debug)]
enum CompiledCurveEasing {
    Linear,
    Hold,
    Bezier(CompiledBezierSegment),
    Steps { steps: f64 },
    Shape { shape: CurveShape, amplitude: f64, phase_cycles: f64, fade_in: f64, fade_out: f64 },
    PerlinNoise { frequency: f64, amplitude: f64, octaves: u32, fade_in: f64, fade_out: f64, phase: f64, seed: u64 },
    Random { frequency: f64, fade_in: f64, fade_out: f64, seed: u64 },
    Script { source: String },
}

impl CompiledCurveEasing {
    fn from_public(easing: &CurveEasing, start: &AnimationCurveKey, end: &AnimationCurveKey) -> Self {
        let span = (end.position - start.position).abs();
        match easing {
            CurveEasing::Linear => Self::Linear,
            CurveEasing::Hold => Self::Hold,
            CurveEasing::Bezier { out_handle, in_handle } => {
                let out_handle = sanitize_handle(*out_handle, default_bezier_out_handle());
                let in_handle = sanitize_handle(*in_handle, default_bezier_in_handle());
                Self::Bezier(CompiledBezierSegment::new(out_handle, in_handle, start, end))
            }
            CurveEasing::Steps { step_mode, step_size, num_steps } => {
                let steps = match step_mode {
                    CurveStepMode::StepSize => {
                        let size = finite_or(*step_size, default_step_size()).abs().max(CURVE_EPSILON);
                        (span / size).ceil().max(1.0)
                    }
                    CurveStepMode::NumSteps => (*num_steps).max(1) as f64,
                };
                Self::Steps { steps }
            }
            CurveEasing::Shape {
                shape,
                amplitude,
                phase_mode,
                frequency,
                num_phases,
                fade_in,
                fade_out,
            } => {
                let phase_cycles = match phase_mode {
                    CurvePhaseMode::Frequency => finite_or(*frequency, default_shape_frequency()).abs() * span,
                    CurvePhaseMode::NumPhases => finite_or(*num_phases, default_shape_num_phases()).abs(),
                };
                Self::Shape {
                    shape: *shape,
                    amplitude: finite_or(*amplitude, default_shape_amplitude()),
                    phase_cycles,
                    fade_in: finite_or(*fade_in, 0.0).max(0.0),
                    fade_out: finite_or(*fade_out, 0.0).max(0.0),
                }
            }
            CurveEasing::PerlinNoise {
                frequency,
                amplitude,
                octaves,
                fade_in,
                fade_out,
                phase,
            } => Self::PerlinNoise {
                frequency: finite_or(*frequency, default_noise_frequency()).abs(),
                amplitude: finite_or(*amplitude, default_noise_amplitude()),
                octaves: (*octaves).clamp(1, MAX_PERLIN_OCTAVES),
                fade_in: finite_or(*fade_in, 0.0).max(0.0),
                fade_out: finite_or(*fade_out, 0.0).max(0.0),
                phase: finite_or(*phase, 0.0),
                seed: stable_mix_u64(start.position.to_bits() ^ end.position.to_bits().rotate_left(17)),
            },
            CurveEasing::Random { frequency, fade_in, fade_out, seed } => Self::Random {
                frequency: finite_or(*frequency, default_random_frequency()).abs(),
                fade_in: finite_or(*fade_in, 0.0).max(0.0),
                fade_out: finite_or(*fade_out, 0.0).max(0.0),
                seed: *seed,
            },
            CurveEasing::Script { source } => Self::Script { source: source.clone() },
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct CubicPolynomial {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
}

impl CubicPolynomial {
    fn from_points(p0: f64, p1: f64, p2: f64, p3: f64) -> Self {
        Self {
            a: -p0 + 3.0 * p1 - 3.0 * p2 + p3,
            b: 3.0 * p0 - 6.0 * p1 + 3.0 * p2,
            c: -3.0 * p0 + 3.0 * p1,
            d: p0,
        }
    }

    fn sample(self, t: f64) -> f64 {
        ((self.a * t + self.b) * t + self.c) * t + self.d
    }

    fn derivative(self, t: f64) -> f64 {
        (3.0 * self.a * t + 2.0 * self.b) * t + self.c
    }
}

#[derive(Clone, Debug)]
struct CompiledBezierSegment {
    x: CubicPolynomial,
    y: CubicPolynomial,
    start_position: f64,
    inv_span: f64,
}

impl CompiledBezierSegment {
    fn new(out_handle: CurveHandle, in_handle: CurveHandle, start: &AnimationCurveKey, end: &AnimationCurveKey) -> Self {
        let span = (end.position - start.position).max(CURVE_EPSILON);
        let value_span = end.value - start.value;

        let out_position = start.position + out_handle.position * span;
        let out_value = start.value + out_handle.value * value_span;
        let in_position = end.position + in_handle.position * span;
        let in_value = end.value + in_handle.value * value_span;

        let p1x = finite_or(out_position, start.position + span / 3.0).clamp(start.position, end.position);
        let p2x = finite_or(in_position, start.position + (span * 2.0) / 3.0).clamp(start.position, end.position);

        let p1y = finite_or(out_value, start.value + value_span / 3.0);
        let p2y = finite_or(in_value, start.value + (value_span * 2.0) / 3.0);

        Self {
            x: CubicPolynomial::from_points(start.position, p1x, p2x, end.position),
            y: CubicPolynomial::from_points(start.value, p1y, p2y, end.value),
            start_position: start.position,
            inv_span: 1.0 / span,
        }
    }

    fn sample(&self, position: f64) -> f64 {
        let mut t = ((position - self.start_position) * self.inv_span).clamp(0.0, 1.0);
        for _ in 0..4 {
            let sampled_x = self.x.sample(t);
            let delta = sampled_x - position;
            if delta.abs() <= 1e-8 {
                break;
            }

            let derivative = self.x.derivative(t);
            if derivative.abs() <= CURVE_EPSILON {
                break;
            }

            t = (t - delta / derivative).clamp(0.0, 1.0);
        }

        let mut low = 0.0;
        let mut high = 1.0;
        for _ in 0..8 {
            let mid = 0.5 * (low + high);
            let sampled_x = self.x.sample(mid);
            if sampled_x <= position {
                low = mid;
            } else {
                high = mid;
            }
        }
        t = 0.5 * (low + high);

        self.y.sample(t)
    }
}

fn sample_compiled_segment(segment: &CompiledCurveSegment, position: f64, script_sampler: &mut Option<&mut dyn CurveScriptSampler>) -> f64 {
    if position <= segment.start_position {
        return segment.start_value;
    }
    if position >= segment.end_position {
        return segment.end_value;
    }

    let progress = ((position - segment.start_position) * segment.inv_span).clamp(0.0, 1.0);
    let linear_value = segment.start_value + segment.value_delta * progress;

    match &segment.easing {
        CompiledCurveEasing::Linear => linear_value,
        CompiledCurveEasing::Hold => segment.start_value,
        CompiledCurveEasing::Bezier(bezier) => bezier.sample(position),
        CompiledCurveEasing::Steps { steps } => {
            let steps = (*steps).max(1.0);
            let step_index = (progress * steps).floor().clamp(0.0, steps);
            let stepped_progress = (step_index / steps).clamp(0.0, 1.0);
            segment.start_value + segment.value_delta * stepped_progress
        }
        CompiledCurveEasing::Shape { shape, amplitude, phase_cycles, fade_in, fade_out } => {
            let envelope = fade_envelope(progress, *fade_in, *fade_out);
            let resolved_amplitude = amplitude * segment.value_delta.abs();
            let wave = sample_shape_wave(*shape, phase_cycles * progress);
            linear_value + resolved_amplitude * wave * envelope
        }
        CompiledCurveEasing::PerlinNoise {
            frequency,
            amplitude,
            octaves,
            fade_in,
            fade_out,
            phase,
            seed,
        } => {
            if *frequency <= CURVE_EPSILON || amplitude.abs() <= CURVE_EPSILON {
                return linear_value;
            }

            let envelope = fade_envelope(progress, *fade_in, *fade_out);
            let noise_position = (position * *frequency) + *phase;
            let noise = fractal_noise_1d(noise_position, *octaves, *seed);
            linear_value + noise * *amplitude * envelope
        }
        CompiledCurveEasing::Random { frequency, fade_in, fade_out, seed } => {
            if *frequency <= CURVE_EPSILON {
                return linear_value;
            }

            let envelope = fade_envelope(progress, *fade_in, *fade_out);
            let bucket = ((position - segment.start_position) * *frequency).floor() as i64;
            let random_progress = hash_to_unit_f64(hash_i64(bucket, *seed));
            let random_value = segment.start_value + segment.value_delta * random_progress;
            linear_value + (random_value - linear_value) * envelope
        }
        CompiledCurveEasing::Script { source } => {
            let context = CurveSampleContext {
                position,
                progress,
                segment_start_position: segment.start_position,
                segment_end_position: segment.end_position,
                segment_start_value: segment.start_value,
                segment_end_value: segment.end_value,
                linear_value,
            };

            if let Some(script_sampler) = script_sampler.as_deref_mut() {
                if let Some(sampled) = script_sampler.sample(source.as_str(), context) {
                    if sampled.is_finite() {
                        return sampled;
                    }
                }
            }

            linear_value
        }
    }
}

fn compile_segments(keys: &[AnimationCurveKey]) -> Vec<CompiledCurveSegment> {
    if keys.len() < 2 {
        return Vec::new();
    }

    let mut compiled = Vec::with_capacity(keys.len() - 1);
    for pair in keys.windows(2) {
        let start = &pair[0];
        let end = &pair[1];
        let span = end.position - start.position;
        if span <= CURVE_EPSILON {
            continue;
        }

        compiled.push(CompiledCurveSegment {
            start_position: start.position,
            end_position: end.position,
            start_value: start.value,
            end_value: end.value,
            value_delta: end.value - start.value,
            inv_span: 1.0 / span,
            easing: CompiledCurveEasing::from_public(&start.easing, start, end),
        });
    }

    compiled
}

fn normalize_keys(keys: &mut Vec<AnimationCurveKey>) {
    keys.retain(|key| key.position.is_finite() && key.value.is_finite());
    keys.sort_by(|a, b| a.position.total_cmp(&b.position));

    if keys.is_empty() {
        return;
    }

    let mut deduplicated = Vec::<AnimationCurveKey>::with_capacity(keys.len());
    for key in keys.drain(..) {
        if let Some(last) = deduplicated.last_mut() {
            if (last.position - key.position).abs() <= CURVE_EPSILON {
                *last = key;
                continue;
            }
        }

        deduplicated.push(key);
    }
    *keys = deduplicated;
}

fn resolve_segment_index(position: f64, segments: &[CompiledCurveSegment]) -> usize {
    segments.partition_point(|segment| segment.end_position <= position).min(segments.len().saturating_sub(1))
}

fn resolve_segment_index_with_cursor(position: f64, segments: &[CompiledCurveSegment], cursor: &mut AnimationCurveCursor) -> usize {
    if segments.is_empty() {
        return 0;
    }

    if !cursor.initialized {
        cursor.index = resolve_segment_index(position, segments);
        cursor.initialized = true;
        return cursor.index;
    }

    cursor.index = cursor.index.min(segments.len().saturating_sub(1));
    while cursor.index + 1 < segments.len() && position >= segments[cursor.index].end_position {
        cursor.index += 1;
    }
    while cursor.index > 0 && position < segments[cursor.index].start_position {
        cursor.index -= 1;
    }

    cursor.index
}

fn sample_shape_wave(shape: CurveShape, phase_cycles: f64) -> f64 {
    let phase = phase_cycles.rem_euclid(1.0);
    match shape {
        CurveShape::Sine => (phase * TAU).sin(),
        CurveShape::Triangle => 1.0 - 4.0 * (phase - 0.5).abs(),
        CurveShape::Saw => 2.0 * phase - 1.0,
        CurveShape::ReverseSaw => 1.0 - 2.0 * phase,
        CurveShape::Square => {
            if phase < 0.5 {
                1.0
            } else {
                -1.0
            }
        }
    }
}

fn fade_envelope(progress: f64, fade_in: f64, fade_out: f64) -> f64 {
    let mut envelope = 1.0_f64;
    if fade_in > CURVE_EPSILON {
        envelope = envelope.min((progress / fade_in).clamp(0.0, 1.0));
    }
    if fade_out > CURVE_EPSILON {
        envelope = envelope.min(((1.0 - progress) / fade_out).clamp(0.0, 1.0));
    }
    envelope.clamp(0.0, 1.0)
}

fn fractal_noise_1d(position: f64, octaves: u32, seed: u64) -> f64 {
    if !position.is_finite() {
        return 0.0;
    }

    let mut frequency = 1.0;
    let mut amplitude = 1.0;
    let mut total = 0.0;
    let mut normalization = 0.0;

    for octave in 0..octaves.max(1) {
        let octave_seed = seed ^ ((octave as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        total += value_noise_1d(position * frequency, octave_seed) * amplitude;
        normalization += amplitude;
        frequency *= 2.0;
        amplitude *= 0.5;
    }

    if normalization <= CURVE_EPSILON { 0.0 } else { total / normalization }
}

fn value_noise_1d(position: f64, seed: u64) -> f64 {
    let left = position.floor() as i64;
    let right = left + 1;
    let local = position - left as f64;
    let weight = smoothstep(local.clamp(0.0, 1.0));

    let left_value = hash_to_unit_f64(hash_i64(left, seed)) * 2.0 - 1.0;
    let right_value = hash_to_unit_f64(hash_i64(right, seed)) * 2.0 - 1.0;
    left_value + (right_value - left_value) * weight
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn hash_i64(value: i64, seed: u64) -> u64 {
    let base = (value as u64).wrapping_add(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    stable_mix_u64(base)
}

fn stable_mix_u64(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^= value >> 33;
    value
}

fn hash_to_unit_f64(hash: u64) -> f64 {
    const UNIT_INV: f64 = 1.0 / ((1u64 << 53) as f64);
    ((hash >> 11) as f64) * UNIT_INV
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    if value.is_finite() { value } else { fallback }
}

fn sanitize_handle(handle: CurveHandle, fallback: CurveHandle) -> CurveHandle {
    CurveHandle {
        position: finite_or(handle.position, fallback.position),
        value: finite_or(handle.value, fallback.value),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64) {
        assert!((actual - expected).abs() <= 1e-6, "expected {expected}, got {actual} (delta={})", (actual - expected).abs());
    }

    #[test]
    fn linear_curve_samples_expected_values() {
        let curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Linear), AnimationCurveKey::new(10.0, 10.0, CurveEasing::Linear)]);

        assert_close(curve.sample(-1.0).expect("sample should exist"), 0.0);
        assert_close(curve.sample(5.0).expect("sample should exist"), 5.0);
        assert_close(curve.sample(15.0).expect("sample should exist"), 10.0);
    }

    #[test]
    fn value_range_constraint_clamps_sampled_output() {
        let mut curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, -10.0, CurveEasing::Linear), AnimationCurveKey::new(1.0, 10.0, CurveEasing::Linear)]);
        curve.set_value_range_constraint(Some(-2.0), Some(3.0));

        assert_close(curve.sample(0.0).expect("sample should exist"), -2.0);
        assert_close(curve.sample(1.0).expect("sample should exist"), 3.0);
        assert_close(curve.sample(0.5).expect("sample should exist"), 0.0);
    }

    #[test]
    fn hold_easing_keeps_start_value_until_boundary() {
        let curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 3.0, CurveEasing::Hold), AnimationCurveKey::new(2.0, 9.0, CurveEasing::Linear)]);

        assert_close(curve.sample(0.5).expect("sample should exist"), 3.0);
        assert_close(curve.sample(1.9).expect("sample should exist"), 3.0);
        assert_close(curve.sample(2.0).expect("sample should exist"), 9.0);
    }

    #[test]
    fn steps_mode_num_steps_quantizes_progress() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(
                0.0,
                0.0,
                CurveEasing::Steps {
                    step_mode: CurveStepMode::NumSteps,
                    step_size: 0.1,
                    num_steps: 4,
                },
            ),
            AnimationCurveKey::new(4.0, 8.0, CurveEasing::Linear),
        ]);

        assert_close(curve.sample(0.9).expect("sample should exist"), 0.0);
        assert_close(curve.sample(1.1).expect("sample should exist"), 2.0);
        assert_close(curve.sample(2.2).expect("sample should exist"), 4.0);
        assert_close(curve.sample(3.8).expect("sample should exist"), 6.0);
    }

    #[test]
    fn bezier_easing_stays_on_key_boundaries() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(
                0.0,
                0.0,
                CurveEasing::Bezier {
                    out_handle: CurveHandle::new(0.2, 0.0),
                    in_handle: CurveHandle::new(-0.2, -0.2),
                },
            ),
            AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear),
        ]);

        assert_close(curve.sample(0.0).expect("sample should exist"), 0.0);
        assert_close(curve.sample(1.0).expect("sample should exist"), 1.0);
    }

    #[test]
    fn bezier_crossed_handle_x_stays_non_linear() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(
                0.0,
                0.0,
                CurveEasing::Bezier {
                    out_handle: CurveHandle::new(1.0, 0.0),
                    in_handle: CurveHandle::new(-1.0, 0.0),
                },
            ),
            AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear),
        ]);

        let quarter = curve.sample(0.25).expect("sample should exist");
        let half = curve.sample(0.5).expect("sample should exist");
        let three_quarter = curve.sample(0.75).expect("sample should exist");

        assert!(quarter < 0.2, "expected strong ease-in shape, got {quarter}");
        assert!((half - 0.5).abs() < 0.01, "expected midpoint to stay near 0.5, got {half}");
        assert!(three_quarter > 0.8, "expected strong ease-out shape, got {three_quarter}");
    }

    #[test]
    fn shape_relative_mode_modulates_linear_segment() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(
                0.0,
                0.0,
                CurveEasing::Shape {
                    shape: CurveShape::Sine,
                    amplitude: 0.5,
                    phase_mode: CurvePhaseMode::NumPhases,
                    frequency: 1.0,
                    num_phases: 1.0,
                    fade_in: 0.0,
                    fade_out: 0.0,
                },
            ),
            AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear),
        ]);

        assert_close(curve.sample(0.25).expect("sample should exist"), 0.75);
    }

    #[test]
    fn perlin_noise_is_deterministic() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(
                0.0,
                0.0,
                CurveEasing::PerlinNoise {
                    frequency: 2.0,
                    amplitude: 1.0,
                    octaves: 5,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    phase: 0.3,
                },
            ),
            AnimationCurveKey::new(2.0, 1.0, CurveEasing::Linear),
        ]);

        let a = curve.sample(0.77).expect("sample should exist");
        let b = curve.sample(0.77).expect("sample should exist");
        assert_close(a, b);
    }

    #[test]
    fn random_mode_uses_seed() {
        let curve_a = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Random { frequency: 10.0, fade_in: 0.0, fade_out: 0.0, seed: 12 }), AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear)]);
        let curve_b = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Random { frequency: 10.0, fade_in: 0.0, fade_out: 0.0, seed: 12 }), AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear)]);
        let curve_c = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Random { frequency: 10.0, fade_in: 0.0, fade_out: 0.0, seed: 42 }), AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear)]);

        let a = curve_a.sample(0.43).expect("sample should exist");
        let b = curve_b.sample(0.43).expect("sample should exist");
        let c = curve_c.sample(0.43).expect("sample should exist");
        assert_close(a, b);
        assert!((a - c).abs() > 1e-6, "different seeds should produce different values");
    }

    #[test]
    fn sample_range_matches_single_point_sampling() {
        let curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Linear), AnimationCurveKey::new(1.0, 1.0, CurveEasing::Hold), AnimationCurveKey::new(2.0, 0.0, CurveEasing::Linear)]);

        let mut sampled = vec![0.0; 257];
        let written = curve.sample_range(0.0, 2.0, sampled.as_mut_slice());
        assert_eq!(written, sampled.len());

        let step = 2.0 / ((sampled.len() - 1) as f64);
        for (index, value) in sampled.iter().enumerate() {
            let position = if index + 1 == sampled.len() { 2.0 } else { step * (index as f64) };
            let expected = curve.sample(position).expect("sample should exist");
            assert_close(*value, expected);
        }
    }

    #[test]
    fn cursor_sampling_matches_regular_sampling_in_both_directions() {
        let curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 1.0, CurveEasing::Linear), AnimationCurveKey::new(1.0, 5.0, CurveEasing::Linear), AnimationCurveKey::new(2.0, 2.0, CurveEasing::Linear)]);

        let mut cursor = AnimationCurveCursor::new();
        for index in 0..200 {
            let position = index as f64 / 100.0;
            let with_cursor = curve.sample_with_cursor(position, &mut cursor).expect("sample should exist");
            let without_cursor = curve.sample(position).expect("sample should exist");
            assert_close(with_cursor, without_cursor);
        }

        for index in (0..200).rev() {
            let position = index as f64 / 100.0;
            let with_cursor = curve.sample_with_cursor(position, &mut cursor).expect("sample should exist");
            let without_cursor = curve.sample(position).expect("sample should exist");
            assert_close(with_cursor, without_cursor);
        }
    }

    struct ScriptSampler {
        calls: usize,
    }

    impl CurveScriptSampler for ScriptSampler {
        fn sample(&mut self, source: &str, context: CurveSampleContext) -> Option<f64> {
            self.calls += 1;
            if source == "offset+2" { Some(context.linear_value + 2.0) } else { None }
        }
    }

    #[test]
    fn script_easing_uses_host_callback() {
        let curve = AnimationCurve::new(vec![AnimationCurveKey::new(0.0, 0.0, CurveEasing::Script { source: "offset+2".to_string() }), AnimationCurveKey::new(10.0, 10.0, CurveEasing::Linear)]);

        let mut sampler = ScriptSampler { calls: 0 };
        let sampled = curve.sample_with_script(5.0, &mut sampler).expect("sample should exist");
        assert_close(sampled, 7.0);
        assert_eq!(sampler.calls, 1);
    }

    #[test]
    fn constructor_sorts_and_deduplicates_keys() {
        let curve = AnimationCurve::new(vec![
            AnimationCurveKey::new(2.0, 2.0, CurveEasing::Linear),
            AnimationCurveKey::new(0.0, 0.0, CurveEasing::Linear),
            AnimationCurveKey::new(1.0, 1.0, CurveEasing::Linear),
            AnimationCurveKey::new(1.0, 7.0, CurveEasing::Hold),
            AnimationCurveKey::new(f64::NAN, 9.0, CurveEasing::Linear),
        ]);

        assert_eq!(curve.key_count(), 3);
        assert_close(curve.keys()[0].position, 0.0);
        assert_close(curve.keys()[1].position, 1.0);
        assert_close(curve.keys()[2].position, 2.0);
        assert_close(curve.keys()[1].value, 7.0);
    }
}
