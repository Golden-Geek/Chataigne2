import type {
	CurveShape,
	ParsedCurveKey,
	CubicPolynomial,
	CompiledSegmentEasing,
	CompiledCurveSegment,
	CompiledCurveKey,
	CompiledCurve
} from './types';
import {
	DECL_KIND,
	DECL_OUT_POSITION,
	DECL_OUT_VALUE,
	DECL_IN_POSITION,
	DECL_IN_VALUE,
	DECL_STEP_MODE,
	DECL_STEP_SIZE,
	DECL_NUM_STEPS,
	DECL_SHAPE,
	DECL_AMPLITUDE,
	DECL_PHASE_MODE,
	DECL_FREQUENCY,
	DECL_NUM_PHASES,
	DECL_FADE_IN,
	DECL_FADE_OUT,
	DECL_OCTAVES,
	DECL_PHASE,
	DECL_SEED,
	DECL_SCRIPT_SOURCE,
	CURVE_EPSILON,
	MAX_PERLIN_OCTAVES
} from './constants';
import {
	clamp,
	finite_or,
	param_number_value,
	param_string_value,
	normalize_easing_kind,
	normalize_step_mode,
	normalize_shape,
	normalize_phase_mode
} from './values';

export const cubic_from_points = (
	p0: number,
	p1: number,
	p2: number,
	p3: number
): CubicPolynomial => ({
	a: -p0 + 3 * p1 - 3 * p2 + p3,
	b: 3 * p0 - 6 * p1 + 3 * p2,
	c: -3 * p0 + 3 * p1,
	d: p0
});
const sample_cubic = (poly: CubicPolynomial, t: number): number =>
	((poly.a * t + poly.b) * t + poly.c) * t + poly.d;
const derivative_cubic = (poly: CubicPolynomial, t: number): number =>
	(3 * poly.a * t + 2 * poly.b) * t + poly.c;

const smoothstep = (value: number): number => value * value * (3 - 2 * value);
const stable_mix_u32 = (value: number): number => {
	let mixed = value >>> 0;
	mixed ^= mixed >>> 16;
	mixed = Math.imul(mixed, 0x7feb352d);
	mixed ^= mixed >>> 15;
	mixed = Math.imul(mixed, 0x846ca68b);
	mixed ^= mixed >>> 16;
	return mixed >>> 0;
};
const hash_i32 = (value: number, seed: number): number => {
	const base = (value | 0) + Math.imul(seed | 0, 0x9e3779b9);
	return stable_mix_u32(base);
};
const hash_to_unit = (hash: number): number => (hash >>> 0) / 0xffffffff;
const value_noise_1d = (position: number, seed: number): number => {
	const left = Math.floor(position);
	const right = left + 1;
	const local = clamp(position - left, 0, 1);
	const weight = smoothstep(local);
	const left_value = hash_to_unit(hash_i32(left, seed)) * 2 - 1;
	const right_value = hash_to_unit(hash_i32(right, seed)) * 2 - 1;
	return left_value + (right_value - left_value) * weight;
};
const fractal_noise_1d = (position: number, octaves: number, seed: number): number => {
	if (!Number.isFinite(position)) {
		return 0;
	}

	let frequency = 1;
	let amplitude = 1;
	let total = 0;
	let normalization = 0;
	const octave_count = Math.max(1, Math.min(MAX_PERLIN_OCTAVES, Math.round(octaves)));
	for (let octave = 0; octave < octave_count; octave += 1) {
		const octave_seed = stable_mix_u32(seed ^ Math.imul(octave + 1, 0x9e3779b9));
		total += value_noise_1d(position * frequency, octave_seed) * amplitude;
		normalization += amplitude;
		frequency *= 2;
		amplitude *= 0.5;
	}
	if (normalization <= CURVE_EPSILON) {
		return 0;
	}
	return total / normalization;
};
const fade_envelope = (progress: number, fade_in: number, fade_out: number): number => {
	let envelope = 1;
	if (fade_in > CURVE_EPSILON) {
		envelope = Math.min(envelope, clamp(progress / fade_in, 0, 1));
	}
	if (fade_out > CURVE_EPSILON) {
		envelope = Math.min(envelope, clamp((1 - progress) / fade_out, 0, 1));
	}
	return clamp(envelope, 0, 1);
};
const elastic_progress = (progress: number): number => {
	if (progress <= 0) {
		return 0;
	}
	if (progress >= 1) {
		return 1;
	}
	const oscillation = (progress * 10 - 0.75) * ((Math.PI * 2) / 3);
	return Math.pow(2, -10 * progress) * Math.sin(oscillation) + 1;
};
const bounce_progress = (progress: number): number => {
	if (progress <= 0) {
		return 0;
	}
	if (progress >= 1) {
		return 1;
	}

	let t = progress;
	const n1 = 7.5625;
	const d1 = 2.75;
	if (t < 1 / d1) {
		return n1 * t * t;
	}
	if (t < 2 / d1) {
		t -= 1.5 / d1;
		return n1 * t * t + 0.75;
	}
	if (t < 2.5 / d1) {
		t -= 2.25 / d1;
		return n1 * t * t + 0.9375;
	}
	t -= 2.625 / d1;
	return n1 * t * t + 0.984375;
};
const shape_wave = (shape: CurveShape, phase_cycles: number): number => {
	const phase = ((phase_cycles % 1) + 1) % 1;
	if (shape === 'triangle') {
		return 1 - 4 * Math.abs(phase - 0.5);
	}
	if (shape === 'saw') {
		return 2 * phase - 1;
	}
	if (shape === 'reverseSaw') {
		return 1 - 2 * phase;
	}
	if (shape === 'square') {
		return phase < 0.5 ? 1 : -1;
	}
	return Math.sin(phase * Math.PI * 2);
};

export const compile_curve = (parsed_keys: ParsedCurveKey[]): CompiledCurve => {
	const normalized: CompiledCurveKey[] = parsed_keys
		.filter((key) => Number.isFinite(key.position) && Number.isFinite(key.value))
		.map((key) => ({
			node_id: key.node_id,
			position: key.position,
			value: key.value,
			easing: key.easing
		}))
		.sort((left, right) => left.position - right.position || left.node_id - right.node_id);

	if (normalized.length > 1) {
		const deduplicated: CompiledCurveKey[] = [];
		for (const key of normalized) {
			const previous = deduplicated[deduplicated.length - 1];
			if (previous && Math.abs(previous.position - key.position) <= CURVE_EPSILON) {
				deduplicated[deduplicated.length - 1] = key;
				continue;
			}
			deduplicated.push(key);
		}
		normalized.splice(0, normalized.length, ...deduplicated);
	}

	const segments: CompiledCurveSegment[] = [];
	for (let index = 0; index + 1 < normalized.length; index += 1) {
		const start = normalized[index];
		const end = normalized[index + 1];
		const span = end.position - start.position;
		if (span <= CURVE_EPSILON) {
			continue;
		}

		const params = start.easing?.params ?? {};
		const easing_kind =
			start.easing?.kind ??
			normalize_easing_kind(param_string_value(params[DECL_KIND]?.value, 'linear'));

		let easing: CompiledSegmentEasing = { kind: 'linear' };
		if (easing_kind === 'elastic') {
			easing = { kind: 'elastic' };
		} else if (easing_kind === 'bounce') {
			easing = { kind: 'bounce' };
		} else if (easing_kind === 'hold') {
			easing = { kind: 'hold' };
		} else if (easing_kind === 'bezier') {
			const out_position = param_number_value(params[DECL_OUT_POSITION]?.value, 1 / 3);
			const out_value = param_number_value(params[DECL_OUT_VALUE]?.value, 1 / 3);
			const in_position = param_number_value(params[DECL_IN_POSITION]?.value, -1 / 3);
			const in_value = param_number_value(params[DECL_IN_VALUE]?.value, -1 / 3);

			const value_span = end.value - start.value;
			let p1x = finite_or(start.position + out_position * span, start.position + span / 3);
			let p2x = finite_or(end.position + in_position * span, start.position + (span * 2) / 3);
			const p1y = finite_or(start.value + out_value, start.value + value_span / 3);
			const p2y = finite_or(end.value + in_value, start.value + (value_span * 2) / 3);
			p1x = clamp(p1x, start.position, end.position);
			p2x = clamp(p2x, start.position, end.position);
			easing = {
				kind: 'bezier',
				x: cubic_from_points(start.position, p1x, p2x, end.position),
				y: cubic_from_points(start.value, p1y, p2y, end.value),
				start_position: start.position,
				inv_span: 1 / span
			};
		} else if (easing_kind === 'steps') {
			const step_mode = normalize_step_mode(
				param_string_value(params[DECL_STEP_MODE]?.value, 'numSteps')
			);
			let steps = Math.max(1, param_number_value(params[DECL_NUM_STEPS]?.value, 8));
			if (step_mode === 'stepSize') {
				const step_size = Math.max(
					CURVE_EPSILON,
					Math.abs(param_number_value(params[DECL_STEP_SIZE]?.value, 0.1))
				);
				steps = Math.max(1, Math.ceil(Math.abs(span) / step_size));
			}
			easing = {
				kind: 'steps',
				steps
			};
		} else if (easing_kind === 'shape') {
			const phase_mode = normalize_phase_mode(
				param_string_value(params[DECL_PHASE_MODE]?.value, 'frequency')
			);
			const shape = normalize_shape(param_string_value(params[DECL_SHAPE]?.value, 'sine'));
			const amplitude = param_number_value(params[DECL_AMPLITUDE]?.value, 1);
			const frequency = Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1));
			const num_phases = Math.abs(param_number_value(params[DECL_NUM_PHASES]?.value, 1));
			const phase_cycles = phase_mode === 'frequency' ? frequency * Math.abs(span) : num_phases;
			easing = {
				kind: 'shape',
				shape,
				amplitude,
				phase_cycles,
				fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
				fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			};
		} else if (easing_kind === 'perlinNoise') {
			easing = {
				kind: 'perlinNoise',
				frequency: Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1)),
				amplitude: param_number_value(params[DECL_AMPLITUDE]?.value, 1),
				octaves: clamp(param_number_value(params[DECL_OCTAVES]?.value, 4), 1, MAX_PERLIN_OCTAVES),
				fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
				fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0)),
				phase: param_number_value(params[DECL_PHASE]?.value, 0),
				seed: 0
			};
		} else if (easing_kind === 'random') {
			easing = {
				kind: 'random',
				frequency: Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 6)),
				fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
				fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0)),
				seed: Math.round(param_number_value(params[DECL_SEED]?.value, 0))
			};
		} else if (easing_kind === 'script') {
			easing = {
				kind: 'script',
				source: param_string_value(params[DECL_SCRIPT_SOURCE]?.value, '')
			};
		}

		segments.push({
			start_position: start.position,
			end_position: end.position,
			start_value: start.value,
			end_value: end.value,
			value_delta: end.value - start.value,
			inv_span: 1 / span,
			easing
		});
	}

	return {
		keys: normalized,
		segments
	};
};

const sample_bezier = (
	position: number,
	x: CubicPolynomial,
	y: CubicPolynomial,
	start_position: number,
	inv_span: number
): number => {
	let t = clamp((position - start_position) * inv_span, 0, 1);
	let converged = false;
	for (let iteration = 0; iteration < 6; iteration += 1) {
		const sampled_x = sample_cubic(x, t);
		const delta = sampled_x - position;
		if (Math.abs(delta) <= 1e-10) {
			converged = true;
			break;
		}
		const derivative = derivative_cubic(x, t);
		if (Math.abs(derivative) <= CURVE_EPSILON) {
			break;
		}
		const next_t = t - delta / derivative;
		if (!Number.isFinite(next_t)) {
			break;
		}
		t = clamp(next_t, 0, 1);
	}

	if (!converged) {
		let low = 0;
		let high = 1;
		for (let iteration = 0; iteration < 20; iteration += 1) {
			const midpoint = (low + high) * 0.5;
			const sampled_x = sample_cubic(x, midpoint);
			if (sampled_x <= position) {
				low = midpoint;
			} else {
				high = midpoint;
			}
		}
		t = (low + high) * 0.5;
	}
	return sample_cubic(y, t);
};

export const solve_bezier_t_for_position = (
	position: number,
	x: CubicPolynomial,
	start_position: number,
	inv_span: number
): number => {
	let t = clamp((position - start_position) * inv_span, 0, 1);
	for (let iteration = 0; iteration < 6; iteration += 1) {
		const sampled_x = sample_cubic(x, t);
		const delta = sampled_x - position;
		if (Math.abs(delta) <= 1e-9) {
			break;
		}
		const derivative = derivative_cubic(x, t);
		if (Math.abs(derivative) <= CURVE_EPSILON) {
			break;
		}
		t = clamp(t - delta / derivative, 0, 1);
	}

	let low = 0;
	let high = 1;
	for (let iteration = 0; iteration < 14; iteration += 1) {
		const midpoint = (low + high) * 0.5;
		const sampled_x = sample_cubic(x, midpoint);
		if (sampled_x <= position) {
			low = midpoint;
		} else {
			high = midpoint;
		}
	}
	return clamp((low + high) * 0.5, 0, 1);
};

const sample_segment = (segment: CompiledCurveSegment, position: number): number => {
	if (position <= segment.start_position) {
		return segment.start_value;
	}
	if (position >= segment.end_position) {
		return segment.end_value;
	}

	const progress = clamp((position - segment.start_position) * segment.inv_span, 0, 1);
	const linear_value = segment.start_value + segment.value_delta * progress;
	const easing = segment.easing;

	if (easing.kind === 'linear') {
		return linear_value;
	}
	if (easing.kind === 'elastic') {
		return segment.start_value + segment.value_delta * elastic_progress(progress);
	}
	if (easing.kind === 'bounce') {
		return segment.start_value + segment.value_delta * bounce_progress(progress);
	}
	if (easing.kind === 'hold') {
		return segment.start_value;
	}
	if (easing.kind === 'bezier') {
		return sample_bezier(position, easing.x, easing.y, easing.start_position, easing.inv_span);
	}
	if (easing.kind === 'steps') {
		const steps = Math.max(1, Math.round(easing.steps));
		const scaled_progress = progress * steps;
		const rounded_progress = Math.round(scaled_progress);
		const snap_tolerance = Number.EPSILON * Math.max(32, Math.abs(scaled_progress) * 64);
		const step_index = clamp(
			Math.abs(scaled_progress - rounded_progress) <= snap_tolerance
				? rounded_progress
				: Math.floor(scaled_progress),
			0,
			steps
		);
		const stepped_progress = clamp(step_index / steps, 0, 1);
		return segment.start_value + segment.value_delta * stepped_progress;
	}
	if (easing.kind === 'shape') {
		const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
		const wave = shape_wave(easing.shape, easing.phase_cycles * progress);
		return linear_value + easing.amplitude * wave * envelope;
	}
	if (easing.kind === 'perlinNoise') {
		if (easing.frequency <= CURVE_EPSILON || Math.abs(easing.amplitude) <= CURVE_EPSILON) {
			return linear_value;
		}
		const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
		const noise_position = (position - segment.start_position) * easing.frequency + easing.phase;
		const noise = fractal_noise_1d(noise_position, easing.octaves, easing.seed);
		return linear_value + noise * easing.amplitude * envelope;
	}
	if (easing.kind === 'random') {
		if (easing.frequency <= CURVE_EPSILON) {
			return linear_value;
		}
		const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
		const bucket = Math.floor((position - segment.start_position) * easing.frequency);
		const random_progress = hash_to_unit(hash_i32(bucket, easing.seed));
		const random_value = segment.start_value + segment.value_delta * random_progress;
		return linear_value + (random_value - linear_value) * envelope;
	}
	return linear_value;
};

export const sample_compiled_curve = (
	compiled_curve: CompiledCurve,
	position: number
): number | null => {
	const keys = compiled_curve.keys;
	if (keys.length === 0 || !Number.isFinite(position)) {
		return null;
	}
	if (keys.length === 1) {
		return keys[0].value;
	}
	const first = keys[0];
	const last = keys[keys.length - 1];
	if (position <= first.position) {
		return first.value;
	}
	if (position >= last.position) {
		return last.value;
	}

	const segments = compiled_curve.segments;
	if (segments.length === 0) {
		return first.value;
	}

	let low = 0;
	let high = segments.length;
	while (low < high) {
		const middle = (low + high) >> 1;
		if (segments[middle].end_position <= position) {
			low = middle + 1;
		} else {
			high = middle;
		}
	}
	const segment_index = Math.min(low, segments.length - 1);
	return sample_segment(segments[segment_index], position);
};
