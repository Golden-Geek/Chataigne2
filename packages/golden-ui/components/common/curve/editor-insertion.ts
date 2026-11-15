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
	param_number_value,
	param_string_value,
	normalize_step_mode,
	normalize_shape,
	normalize_phase_mode
} from './values';
import { cubic_from_points, solve_bezier_t_for_position } from './evaluation';
import type { NodeId, ParamValue } from '../../../types';
import type {
	ParsedCurveKey,
	BezierHandleControl,
	BezierEasingPreview,
	CurveCreatePlan,
	PendingSplitEasing
} from './types';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorSculpt } from './editor-sculpt';
interface Context {
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly sculpt: ReturnType<typeof createCurveEditorSculpt>;
}
export function createCurveEditorInsertion(context: Context) {
	const enum_param_value = (value: string): ParamValue => ({ kind: 'enum', value });
	const float_param_value = (value: number): ParamValue => ({ kind: 'float', value });
	const str_param_value = (value: string): ParamValue => ({ kind: 'str', value });
	const bezier_preview_param_values = (
		preview: BezierEasingPreview
	): Record<string, ParamValue> => ({
		[DECL_OUT_POSITION]: float_param_value(preview.out_position),
		[DECL_OUT_VALUE]: float_param_value(preview.out_value),
		[DECL_IN_POSITION]: float_param_value(preview.in_position),
		[DECL_IN_VALUE]: float_param_value(preview.in_value)
	});
	const set_split_numeric_param = (
		target: Record<string, ParamValue>,
		decl_id: string,
		value: number
	): void => {
		if (!Number.isFinite(value)) {
			return;
		}
		target[decl_id] = float_param_value(value);
	};
	const find_split_source_segment = (
		position: number,
		preferred_owner_key_id: NodeId | null
	): {
		start: ParsedCurveKey;
		end: ParsedCurveKey;
	} | null => {
		if (!Number.isFinite(position) || context.model.parsed_keys.length < 2) {
			return null;
		}
		let fallback: {
			start: ParsedCurveKey;
			end: ParsedCurveKey;
		} | null = null;
		for (let index = 0; index + 1 < context.model.parsed_keys.length; index += 1) {
			const start = context.model.parsed_keys[index];
			const end = context.model.parsed_keys[index + 1];
			const span = end.position - start.position;
			if (!(span > CURVE_EPSILON)) {
				continue;
			}
			if (position <= start.position + CURVE_EPSILON || position >= end.position - CURVE_EPSILON) {
				continue;
			}
			const candidate = { start, end };
			if (preferred_owner_key_id !== null && start.node_id === preferred_owner_key_id) {
				return candidate;
			}
			if (!fallback) {
				fallback = candidate;
			}
		}
		return fallback;
	};
	const build_split_easing = (
		start: ParsedCurveKey,
		end: ParsedCurveKey,
		split_position: number,
		split_value: number
	): PendingSplitEasing | null => {
		const span = end.position - start.position;
		if (!(span > CURVE_EPSILON)) {
			return null;
		}
		const ratio = clamp((split_position - start.position) / span, 0, 1);
		if (ratio <= CURVE_EPSILON || ratio >= 1 - CURVE_EPSILON) {
			return null;
		}
		const easing_kind = start.easing?.kind ?? 'linear';
		const params = start.easing?.params ?? {};
		const left_param_values: Record<string, ParamValue> = {
			[DECL_KIND]: enum_param_value(easing_kind)
		};
		const right_param_values: Record<string, ParamValue> = {
			[DECL_KIND]: enum_param_value(easing_kind)
		};
		if (easing_kind === 'bezier') {
			const segment = context.model.resolve_bezier_handle_segment(start, end);
			if (!segment) {
				return null;
			}
			const left_span = split_position - start.position;
			const right_span = end.position - split_position;
			if (!(left_span > CURVE_EPSILON) || !(right_span > CURVE_EPSILON)) {
				return null;
			}
			const x_curve = cubic_from_points(
				start.position,
				segment.out_curve_position,
				segment.in_curve_position,
				end.position
			);
			const t = solve_bezier_t_for_position(split_position, x_curve, start.position, 1 / span);
			const mix = (left: number, right: number): number => left + (right - left) * t;
			const point_a_x = mix(start.position, segment.out_curve_position);
			const point_a_y = mix(start.value, segment.out_curve_value);
			const point_b_x = mix(segment.out_curve_position, segment.in_curve_position);
			const point_b_y = mix(segment.out_curve_value, segment.in_curve_value);
			const point_c_x = mix(segment.in_curve_position, end.position);
			const point_c_y = mix(segment.in_curve_value, end.value);
			const point_d_x = mix(point_a_x, point_b_x);
			const point_d_y = mix(point_a_y, point_b_y);
			const point_e_x = mix(point_b_x, point_c_x);
			const point_e_y = mix(point_b_y, point_c_y);
			const left_out_position = (point_a_x - start.position) / left_span;
			const left_in_position = (point_d_x - split_position) / left_span;
			const right_out_position = (point_e_x - split_position) / right_span;
			const right_in_position = (point_c_x - end.position) / right_span;
			set_split_numeric_param(left_param_values, DECL_OUT_POSITION, left_out_position);
			set_split_numeric_param(left_param_values, DECL_OUT_VALUE, point_a_y - start.value);
			set_split_numeric_param(left_param_values, DECL_IN_POSITION, left_in_position);
			set_split_numeric_param(left_param_values, DECL_IN_VALUE, point_d_y - split_value);
			set_split_numeric_param(right_param_values, DECL_OUT_POSITION, right_out_position);
			set_split_numeric_param(right_param_values, DECL_OUT_VALUE, point_e_y - split_value);
			set_split_numeric_param(right_param_values, DECL_IN_POSITION, right_in_position);
			set_split_numeric_param(right_param_values, DECL_IN_VALUE, point_c_y - end.value);
			const preserved_aligned_handles = new Map<NodeId, BezierEasingPreview>();
			const start_alignment = context.sculpt.resolve_curve_drag_opposite_alignment(start.node_id, {
				key_id: start.node_id,
				kind: 'out'
			});
			if (start_alignment) {
				context.sculpt.apply_curve_drag_opposite_alignment(
					preserved_aligned_handles,
					start_alignment,
					point_a_x,
					point_a_y,
					start.position,
					start.value
				);
			}
			const end_alignment = context.sculpt.resolve_curve_drag_opposite_alignment(end.node_id, {
				key_id: start.node_id,
				kind: 'in'
			});
			if (end_alignment) {
				context.sculpt.apply_curve_drag_opposite_alignment(
					preserved_aligned_handles,
					end_alignment,
					point_c_x,
					point_c_y,
					end.position,
					end.value
				);
			}
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values,
				preserved_aligned_handle_updates: [...preserved_aligned_handles.entries()].map(
					([key_id, preview]) => ({
						key_id,
						values: bezier_preview_param_values(preview)
					})
				)
			};
		}
		if (easing_kind === 'steps') {
			const step_mode = normalize_step_mode(
				param_string_value(params[DECL_STEP_MODE]?.value, 'numSteps')
			);
			left_param_values[DECL_STEP_MODE] = enum_param_value(step_mode);
			right_param_values[DECL_STEP_MODE] = enum_param_value(step_mode);
			if (step_mode === 'stepSize') {
				const step_size = Math.max(
					CURVE_EPSILON,
					Math.abs(param_number_value(params[DECL_STEP_SIZE]?.value, 0.1))
				);
				set_split_numeric_param(left_param_values, DECL_STEP_SIZE, step_size);
				set_split_numeric_param(right_param_values, DECL_STEP_SIZE, step_size);
			} else {
				const total_steps = Math.max(
					1,
					Math.round(param_number_value(params[DECL_NUM_STEPS]?.value, 8))
				);
				let left_steps = total_steps;
				let right_steps = total_steps;
				if (total_steps > 1) {
					left_steps = clamp(Math.round(total_steps * ratio), 1, total_steps - 1);
					right_steps = Math.max(1, total_steps - left_steps);
				}
				set_split_numeric_param(left_param_values, DECL_NUM_STEPS, left_steps);
				set_split_numeric_param(right_param_values, DECL_NUM_STEPS, right_steps);
			}
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values
			};
		}
		if (easing_kind === 'shape') {
			const shape = normalize_shape(param_string_value(params[DECL_SHAPE]?.value, 'sine'));
			const phase_mode = normalize_phase_mode(
				param_string_value(params[DECL_PHASE_MODE]?.value, 'frequency')
			);
			left_param_values[DECL_SHAPE] = enum_param_value(shape);
			right_param_values[DECL_SHAPE] = enum_param_value(shape);
			left_param_values[DECL_PHASE_MODE] = enum_param_value(phase_mode);
			right_param_values[DECL_PHASE_MODE] = enum_param_value(phase_mode);
			set_split_numeric_param(
				left_param_values,
				DECL_AMPLITUDE,
				param_number_value(params[DECL_AMPLITUDE]?.value, 1)
			);
			set_split_numeric_param(
				right_param_values,
				DECL_AMPLITUDE,
				param_number_value(params[DECL_AMPLITUDE]?.value, 1)
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			if (phase_mode === 'frequency') {
				const frequency = Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1));
				set_split_numeric_param(left_param_values, DECL_FREQUENCY, frequency);
				set_split_numeric_param(right_param_values, DECL_FREQUENCY, frequency);
			} else {
				const num_phases = Math.abs(param_number_value(params[DECL_NUM_PHASES]?.value, 1));
				const left_num_phases = num_phases * ratio;
				const right_num_phases = Math.max(0, num_phases - left_num_phases);
				set_split_numeric_param(left_param_values, DECL_NUM_PHASES, left_num_phases);
				set_split_numeric_param(right_param_values, DECL_NUM_PHASES, right_num_phases);
			}
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values
			};
		}
		if (easing_kind === 'perlinNoise') {
			set_split_numeric_param(
				left_param_values,
				DECL_FREQUENCY,
				Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FREQUENCY,
				Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_AMPLITUDE,
				param_number_value(params[DECL_AMPLITUDE]?.value, 1)
			);
			set_split_numeric_param(
				right_param_values,
				DECL_AMPLITUDE,
				param_number_value(params[DECL_AMPLITUDE]?.value, 1)
			);
			set_split_numeric_param(
				left_param_values,
				DECL_OCTAVES,
				clamp(param_number_value(params[DECL_OCTAVES]?.value, 4), 1, MAX_PERLIN_OCTAVES)
			);
			set_split_numeric_param(
				right_param_values,
				DECL_OCTAVES,
				clamp(param_number_value(params[DECL_OCTAVES]?.value, 4), 1, MAX_PERLIN_OCTAVES)
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_PHASE,
				param_number_value(params[DECL_PHASE]?.value, 0)
			);
			set_split_numeric_param(
				right_param_values,
				DECL_PHASE,
				param_number_value(params[DECL_PHASE]?.value, 0)
			);
			set_split_numeric_param(
				left_param_values,
				DECL_SEED,
				Math.round(param_number_value(params[DECL_SEED]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_SEED,
				Math.round(param_number_value(params[DECL_SEED]?.value, 0))
			);
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values
			};
		}
		if (easing_kind === 'random') {
			set_split_numeric_param(
				left_param_values,
				DECL_FREQUENCY,
				Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 6))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FREQUENCY,
				Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 6))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_IN,
				Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_FADE_OUT,
				Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
			);
			set_split_numeric_param(
				left_param_values,
				DECL_SEED,
				Math.round(param_number_value(params[DECL_SEED]?.value, 0))
			);
			set_split_numeric_param(
				right_param_values,
				DECL_SEED,
				Math.round(param_number_value(params[DECL_SEED]?.value, 0))
			);
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values
			};
		}
		if (easing_kind === 'script') {
			const source = param_string_value(params[DECL_SCRIPT_SOURCE]?.value, '');
			left_param_values[DECL_SCRIPT_SOURCE] = str_param_value(source);
			right_param_values[DECL_SCRIPT_SOURCE] = str_param_value(source);
			return {
				source_start_key_id: start.node_id,
				source_end_key_id: end.node_id,
				left_param_values,
				right_param_values
			};
		}
		return {
			source_start_key_id: start.node_id,
			source_end_key_id: end.node_id,
			left_param_values,
			right_param_values
		};
	};
	const has_easing_param_targets = (
		key: ParsedCurveKey | undefined,
		values: Record<string, ParamValue> | null | undefined
	): boolean => {
		if (!values || Object.keys(values).length === 0) {
			return true;
		}
		if (!key?.easing) {
			return false;
		}
		const params = key.easing.params;
		for (const decl_id of Object.keys(values)) {
			if (!params[decl_id]) {
				return false;
			}
		}
		return true;
	};
	const pending_create_targets_ready = (
		pending: CurveCreatePlan,
		created_key: ParsedCurveKey,
		keys: Map<NodeId, ParsedCurveKey>
	): boolean => {
		if (!created_key.position_param || !created_key.value_param) {
			return false;
		}
		if (pending.split_easing) {
			const source_key = keys.get(pending.split_easing.source_start_key_id);
			if (!has_easing_param_targets(source_key, pending.split_easing.left_param_values)) {
				return false;
			}
			if (!has_easing_param_targets(created_key, pending.split_easing.right_param_values)) {
				return false;
			}
			for (const update of pending.split_easing.preserved_aligned_handle_updates ?? []) {
				if (!has_easing_param_targets(keys.get(update.key_id), update.values)) {
					return false;
				}
			}
		}
		if (!has_easing_param_targets(created_key, pending.easing_values)) {
			return false;
		}
		for (const update of pending.existing_easing_updates) {
			if (!has_easing_param_targets(keys.get(update.key_id), update.values)) {
				return false;
			}
		}
		return true;
	};
	const slope_from_bezier_control = (control: BezierHandleControl | null): number | null => {
		if (!control) {
			return null;
		}
		const delta_x = control.handle_position - control.anchor_position;
		if (Math.abs(delta_x) <= CURVE_EPSILON) {
			return null;
		}
		const slope = (control.handle_value - control.anchor_value) / delta_x;
		return Number.isFinite(slope) ? slope : null;
	};
	const build_bezier_segment_param_values = (
		start_position: number,
		start_value: number,
		end_position: number,
		end_value: number,
		start_slope: number | null,
		end_slope: number | null
	): Record<string, ParamValue> | null => {
		const span = end_position - start_position;
		if (!(span > CURVE_EPSILON)) {
			return null;
		}
		const secant_slope = (end_value - start_value) / span;
		const safe_start_slope = Number.isFinite(start_slope ?? Number.NaN)
			? (start_slope as number)
			: secant_slope;
		const safe_end_slope = Number.isFinite(end_slope ?? Number.NaN)
			? (end_slope as number)
			: secant_slope;
		const handle_delta_position = span / 3;
		return {
			[DECL_KIND]: enum_param_value('bezier'),
			[DECL_OUT_POSITION]: float_param_value(1 / 3),
			[DECL_OUT_VALUE]: float_param_value(safe_start_slope * handle_delta_position),
			[DECL_IN_POSITION]: float_param_value(-1 / 3),
			[DECL_IN_VALUE]: float_param_value(-safe_end_slope * handle_delta_position)
		};
	};
	const build_middle_insert_easing = (
		start: ParsedCurveKey,
		end: ParsedCurveKey,
		position: number,
		value: number
	): PendingSplitEasing | null => {
		const left_span = position - start.position;
		const right_span = end.position - position;
		if (!(left_span > CURVE_EPSILON) || !(right_span > CURVE_EPSILON)) {
			return null;
		}
		const left_secant_slope = (value - start.value) / left_span;
		const right_secant_slope = (end.value - value) / right_span;
		const center_slope =
			Number.isFinite(left_secant_slope) && Number.isFinite(right_secant_slope)
				? (left_secant_slope + right_secant_slope) * 0.5
				: (end.value - start.value) / Math.max(CURVE_EPSILON, end.position - start.position);
		const start_slope =
			slope_from_bezier_control(
				context.sculpt.bezier_control_for_anchor_kind(start.node_id, 'out')
			) ?? left_secant_slope;
		const end_slope =
			slope_from_bezier_control(context.sculpt.bezier_control_for_anchor_kind(end.node_id, 'in')) ??
			right_secant_slope;
		const left_param_values = build_bezier_segment_param_values(
			start.position,
			start.value,
			position,
			value,
			start_slope,
			center_slope
		);
		const right_param_values = build_bezier_segment_param_values(
			position,
			value,
			end.position,
			end.value,
			center_slope,
			end_slope
		);
		if (!left_param_values || !right_param_values) {
			return null;
		}
		return {
			source_start_key_id: start.node_id,
			source_end_key_id: end.node_id,
			left_param_values,
			right_param_values
		};
	};
	const build_endpoint_extension_easing = (
		position: number,
		value: number
	): {
		new_key_easing_values: Record<string, ParamValue> | null;
		existing_easing_updates: Array<{
			key_id: NodeId;
			values: Record<string, ParamValue>;
		}>;
	} => {
		if (context.model.parsed_keys.length === 0) {
			return {
				new_key_easing_values: null,
				existing_easing_updates: []
			};
		}
		const first_key = context.model.parsed_keys[0];
		if (position < first_key.position - CURVE_EPSILON) {
			const secant_slope =
				(first_key.value - value) / Math.max(CURVE_EPSILON, first_key.position - position);
			const end_slope =
				slope_from_bezier_control(
					context.sculpt.bezier_control_for_anchor_kind(first_key.node_id, 'out')
				) ?? secant_slope;
			return {
				new_key_easing_values: build_bezier_segment_param_values(
					position,
					value,
					first_key.position,
					first_key.value,
					secant_slope,
					end_slope
				),
				existing_easing_updates: []
			};
		}
		const last_key = context.model.parsed_keys[context.model.parsed_keys.length - 1];
		if (position > last_key.position + CURVE_EPSILON) {
			const secant_slope =
				(value - last_key.value) / Math.max(CURVE_EPSILON, position - last_key.position);
			const start_slope =
				slope_from_bezier_control(
					context.sculpt.bezier_control_for_anchor_kind(last_key.node_id, 'in')
				) ?? secant_slope;
			const update_values = build_bezier_segment_param_values(
				last_key.position,
				last_key.value,
				position,
				value,
				start_slope,
				secant_slope
			);
			return {
				new_key_easing_values: null,
				existing_easing_updates:
					update_values === null ? [] : [{ key_id: last_key.node_id, values: update_values }]
			};
		}
		return {
			new_key_easing_values: null,
			existing_easing_updates: []
		};
	};
	return {
		enum_param_value,
		float_param_value,
		str_param_value,
		bezier_preview_param_values,
		set_split_numeric_param,
		find_split_source_segment,
		build_split_easing,
		has_easing_param_targets,
		pending_create_targets_ready,
		slope_from_bezier_control,
		build_bezier_segment_param_values,
		build_middle_insert_easing,
		build_endpoint_extension_easing
	};
}
