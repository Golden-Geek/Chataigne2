import type { NodeId } from '../../../types';
import type { CompiledCurve, CompiledCurveSegment } from './types';
import { clamp } from './values';
import { CURVE_EPSILON } from './constants';

const CURVE_RENDER_TARGET_SAMPLE_SPACING_PX = 3.25;
const CURVE_RENDER_MAX_CHORD_ERROR_PX = 0.24;
const CURVE_RENDER_MAX_SEGMENT_SAMPLES = 4096;
const CURVE_RENDER_MAX_SUBDIVISION_DEPTH = 14;

const point_to_chord_distance_sq = (
	point_x: number,
	point_y: number,
	start_x: number,
	start_y: number,
	end_x: number,
	end_y: number
): number => {
	const delta_x = end_x - start_x;
	const delta_y = end_y - start_y;
	const chord_length_sq = delta_x * delta_x + delta_y * delta_y;
	if (chord_length_sq <= CURVE_EPSILON) {
		const point_delta_x = point_x - start_x;
		const point_delta_y = point_y - start_y;
		return point_delta_x * point_delta_x + point_delta_y * point_delta_y;
	}
	const numerator = delta_x * (start_y - point_y) - (start_x - point_x) * delta_y;
	return (numerator * numerator) / chord_length_sq;
};

export const buildCurveSamples = (
	x_min: number,
	x_max: number,
	plot_width: number,
	compiled_curve: CompiledCurve,
	to_screen_x: (position: number) => number,
	to_screen_y: (value: number) => number,
	sample_value: (position: number) => number
): {
	positions: Float64Array;
	values: Float64Array;
	screen_x: Float64Array;
	screen_y: Float64Array;
	edge_owner_key_ids: Array<NodeId | null>;
} => {
	const sample_cache = new Map<number, number>();
	type SampledCurvePoint = {
		position: number;
		value: number;
		screen_x: number;
		screen_y: number;
	};
	const sampled_point_cache = new Map<number, SampledCurvePoint>();
	const sample_at = (position: number): number => {
		const cached = sample_cache.get(position);
		if (cached !== undefined) {
			return cached;
		}
		const sampled = sample_value(position);
		sample_cache.set(position, sampled);
		return sampled;
	};
	const sample_point = (position: number): SampledCurvePoint => {
		const cached = sampled_point_cache.get(position);
		if (cached) {
			return cached;
		}
		const sampled: SampledCurvePoint = {
			position,
			value: sample_at(position),
			screen_x: to_screen_x(position),
			screen_y: to_screen_y(sample_at(position))
		};
		sampled_point_cache.set(position, sampled);
		return sampled;
	};

	const positions: number[] = [];
	const values: number[] = [];
	const screen_x: number[] = [];
	const screen_y: number[] = [];
	const edge_owner_key_ids: Array<NodeId | null> = [];
	const step_value_at_index = (
		segment: CompiledCurveSegment,
		steps: number,
		step_index: number
	): number => {
		const stepped_progress = clamp(step_index / Math.max(1, steps), 0, 1);
		return segment.start_value + (segment.end_value - segment.start_value) * stepped_progress;
	};
	const append_point = (
		position: number,
		value: number,
		edge_owner_key_id: NodeId | null
	): boolean => {
		const last_index = positions.length - 1;
		if (
			last_index >= 0 &&
			Math.abs(positions[last_index] - position) <= CURVE_EPSILON &&
			Math.abs(values[last_index] - value) <= CURVE_EPSILON
		) {
			positions[last_index] = position;
			values[last_index] = value;
			return false;
		}
		positions.push(position);
		values.push(value);
		screen_x.push(to_screen_x(position));
		screen_y.push(to_screen_y(value));
		if (positions.length > 1) {
			edge_owner_key_ids.push(edge_owner_key_id);
		}
		return true;
	};
	const append_sampled_point = (
		point: SampledCurvePoint,
		edge_owner_key_id: NodeId | null
	): boolean => {
		const last_index = positions.length - 1;
		if (
			last_index >= 0 &&
			Math.abs(positions[last_index] - point.position) <= CURVE_EPSILON &&
			Math.abs(values[last_index] - point.value) <= CURVE_EPSILON
		) {
			positions[last_index] = point.position;
			values[last_index] = point.value;
			screen_x[last_index] = point.screen_x;
			screen_y[last_index] = point.screen_y;
			return false;
		}
		positions.push(point.position);
		values.push(point.value);
		screen_x.push(point.screen_x);
		screen_y.push(point.screen_y);
		if (positions.length > 1) {
			edge_owner_key_ids.push(edge_owner_key_id);
		}
		return true;
	};

	const keys = compiled_curve.keys;
	const segments = compiled_curve.segments;
	if (segments.length === 0) {
		append_point(x_min, sample_at(x_min), null);
		append_point(x_max, sample_at(x_max), null);
		return {
			positions: Float64Array.from(positions),
			values: Float64Array.from(values),
			screen_x: Float64Array.from(screen_x),
			screen_y: Float64Array.from(screen_y),
			edge_owner_key_ids
		};
	}

	const first_key = keys[0];
	const last_key = keys[keys.length - 1];
	if (first_key && x_min < first_key.position - CURVE_EPSILON) {
		append_point(x_min, sample_at(x_min), null);
		append_point(Math.min(x_max, first_key.position), first_key.value, null);
	}

	for (let segment_index = 0; segment_index < segments.length; segment_index += 1) {
		const segment = segments[segment_index];
		const owner_key_id = keys[segment_index]?.node_id ?? null;
		const visible_start = Math.max(x_min, segment.start_position);
		const visible_end = Math.min(x_max, segment.end_position);
		if (!(visible_end > visible_start + CURVE_EPSILON) || owner_key_id === null) {
			continue;
		}

		const span = Math.max(CURVE_EPSILON, segment.end_position - segment.start_position);
		const epsilon = Math.max(CURVE_EPSILON, span * 1e-9);
		let segment_point_count = 0;
		const append_segment_point = (point: SampledCurvePoint): void => {
			if (append_sampled_point(point, owner_key_id)) {
				segment_point_count += 1;
			}
		};
		const visible_start_point = sample_point(visible_start);
		const visible_end_point = sample_point(visible_end);
		append_segment_point(visible_start_point);

		if (segment.easing.kind === 'hold') {
			append_point(visible_end, segment.start_value, owner_key_id);
			if (visible_end >= segment.end_position - CURVE_EPSILON) {
				append_point(segment.end_position, segment.end_value, owner_key_id);
			}
			continue;
		}

		if (segment.easing.kind === 'steps') {
			const steps = Math.max(1, Math.round(segment.easing.steps));
			for (let step_index = 1; step_index <= steps; step_index += 1) {
				const boundary_position = segment.start_position + (span * step_index) / Math.max(1, steps);
				if (
					boundary_position <= visible_start + epsilon ||
					boundary_position > visible_end + epsilon
				) {
					continue;
				}
				append_point(
					boundary_position,
					step_value_at_index(segment, steps, step_index - 1),
					owner_key_id
				);
				append_point(
					boundary_position,
					step_value_at_index(segment, steps, step_index),
					owner_key_id
				);
			}
			append_point(visible_end, sample_at(visible_end), owner_key_id);
			continue;
		}

		if (segment.easing.kind === 'linear') {
			append_segment_point(visible_end_point);
			continue;
		}

		const append_adaptive_segment = (
			start_point: SampledCurvePoint,
			end_point: SampledCurvePoint,
			depth: number
		): void => {
			if (segment_point_count >= CURVE_RENDER_MAX_SEGMENT_SAMPLES) {
				append_segment_point(end_point);
				return;
			}

			const position_span = end_point.position - start_point.position;
			if (
				depth >= CURVE_RENDER_MAX_SUBDIVISION_DEPTH ||
				!Number.isFinite(position_span) ||
				position_span <= CURVE_EPSILON
			) {
				append_segment_point(end_point);
				return;
			}

			const quarter_point = sample_point(start_point.position + position_span * 0.25);
			const midpoint = sample_point(start_point.position + position_span * 0.5);
			const three_quarter_point = sample_point(start_point.position + position_span * 0.75);
			const chord_error_sq = Math.max(
				point_to_chord_distance_sq(
					quarter_point.screen_x,
					quarter_point.screen_y,
					start_point.screen_x,
					start_point.screen_y,
					end_point.screen_x,
					end_point.screen_y
				),
				point_to_chord_distance_sq(
					midpoint.screen_x,
					midpoint.screen_y,
					start_point.screen_x,
					start_point.screen_y,
					end_point.screen_x,
					end_point.screen_y
				),
				point_to_chord_distance_sq(
					three_quarter_point.screen_x,
					three_quarter_point.screen_y,
					start_point.screen_x,
					start_point.screen_y,
					end_point.screen_x,
					end_point.screen_y
				)
			);
			const screen_span_x = Math.abs(end_point.screen_x - start_point.screen_x);
			const needs_subdivision =
				screen_span_x > CURVE_RENDER_TARGET_SAMPLE_SPACING_PX ||
				chord_error_sq > CURVE_RENDER_MAX_CHORD_ERROR_PX * CURVE_RENDER_MAX_CHORD_ERROR_PX;
			if (!needs_subdivision) {
				append_segment_point(end_point);
				return;
			}

			append_adaptive_segment(start_point, midpoint, depth + 1);
			append_adaptive_segment(midpoint, end_point, depth + 1);
		};

		append_adaptive_segment(visible_start_point, visible_end_point, 0);
	}

	if (last_key && x_max > last_key.position + CURVE_EPSILON) {
		append_point(Math.max(x_min, last_key.position), last_key.value, null);
		append_point(x_max, sample_at(x_max), null);
	}

	return {
		positions: Float64Array.from(positions),
		values: Float64Array.from(values),
		screen_x: Float64Array.from(screen_x),
		screen_y: Float64Array.from(screen_y),
		edge_owner_key_ids
	};
};
