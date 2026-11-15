import {
	CURVE_EPSILON,
	KEY_HIT_RADIUS_REM,
	CURVE_HIT_RADIUS_REM,
	BEZIER_HANDLE_HIT_RADIUS_REM
} from './constants';
import { clamp } from './values';
import type { NodeId } from '../../../types';
import type {
	ActiveKeyDrag,
	KeyDragPositionConstraint,
	KeyDragBezierAlignmentConstraint
} from './types';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorState } from './editor-state.svelte';
interface Context {
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly state: ReturnType<typeof createCurveEditorState>;
}
export function createCurveEditorConstraints(context: Context) {
	const clamp_curve_position_to_active_range = (position: number): number => {
		const active_range = context.model.active_curve_range_constraint;
		if (!active_range || !Number.isFinite(position)) {
			return position;
		}
		return clamp(position, active_range.x_min, active_range.x_max);
	};
	const clamp_curve_value_to_active_range = (value: number): number => {
		const active_range = context.model.active_curve_range_constraint;
		if (!active_range || !Number.isFinite(value)) {
			return value;
		}
		return clamp(value, active_range.y_min, active_range.y_max);
	};
	const clamp_curve_point_to_active_range = (
		position: number,
		value: number
	): {
		position: number;
		value: number;
	} => ({
		position: clamp_curve_position_to_active_range(position),
		value: clamp_curve_value_to_active_range(value)
	});
	const key_hit_threshold_px = (): number =>
		Math.max(8, context.state.rem_base_px * KEY_HIT_RADIUS_REM);
	const curve_hit_threshold_px = (): number =>
		Math.max(7, context.state.rem_base_px * CURVE_HIT_RADIUS_REM);
	const bezier_handle_hit_threshold_px = (): number =>
		Math.max(8, context.state.rem_base_px * BEZIER_HANDLE_HIT_RADIUS_REM);
	const HANDLE_ALIGNMENT_COSINE_TOLERANCE = 0.035;
	const KEY_NEIGHBOR_POSITION_GAP = 1e-8;
	const build_key_drag_position_delta_constraint = (
		origins: Array<{
			key_id: NodeId;
			position: number;
			value: number;
		}>
	): KeyDragPositionConstraint | null => {
		if (origins.length === 0 || context.model.parsed_keys.length === 0) {
			return null;
		}
		const selected_key_set = new Set<NodeId>(origins.map((entry) => entry.key_id));
		const key_index_by_id = new Map<NodeId, number>();
		for (let index = 0; index < context.model.parsed_keys.length; index += 1) {
			key_index_by_id.set(context.model.parsed_keys[index].node_id, index);
		}
		let min_delta_position = Number.NEGATIVE_INFINITY;
		let max_delta_position = Number.POSITIVE_INFINITY;
		let has_constraint = false;
		const active_range = context.model.active_curve_range_constraint;
		for (const origin of origins) {
			const index = key_index_by_id.get(origin.key_id);
			if (index === undefined) {
				continue;
			}
			const previous = index > 0 ? context.model.parsed_keys[index - 1] : null;
			if (previous && !selected_key_set.has(previous.node_id)) {
				min_delta_position = Math.max(
					min_delta_position,
					previous.position + KEY_NEIGHBOR_POSITION_GAP - origin.position
				);
				has_constraint = true;
			}
			const next =
				index + 1 < context.model.parsed_keys.length ? context.model.parsed_keys[index + 1] : null;
			if (next && !selected_key_set.has(next.node_id)) {
				max_delta_position = Math.min(
					max_delta_position,
					next.position - KEY_NEIGHBOR_POSITION_GAP - origin.position
				);
				has_constraint = true;
			}
			if (active_range) {
				min_delta_position = Math.max(min_delta_position, active_range.x_min - origin.position);
				max_delta_position = Math.min(max_delta_position, active_range.x_max - origin.position);
				has_constraint = true;
			}
		}
		if (!has_constraint) {
			return null;
		}
		if (min_delta_position > max_delta_position) {
			const locked_delta =
				min_delta_position > 0 && max_delta_position < 0
					? 0
					: Math.abs(min_delta_position) <= Math.abs(max_delta_position)
						? min_delta_position
						: max_delta_position;
			return {
				min_delta_position: locked_delta,
				max_delta_position: locked_delta
			};
		}
		return {
			min_delta_position,
			max_delta_position
		};
	};
	const clamp_key_drag_position_delta = (drag: ActiveKeyDrag, delta_position: number): number => {
		const constraint = drag.position_delta_constraint;
		if (!constraint) {
			return delta_position;
		}
		return clamp(delta_position, constraint.min_delta_position, constraint.max_delta_position);
	};
	const build_key_drag_bezier_alignment_constraints = (
		drag_key_ids: NodeId[]
	): KeyDragBezierAlignmentConstraint[] => {
		if (drag_key_ids.length === 0 || context.model.parsed_keys.length < 3) {
			return [];
		}
		const index_by_key_id = new Map<NodeId, number>();
		for (let index = 0; index < context.model.parsed_keys.length; index += 1) {
			index_by_key_id.set(context.model.parsed_keys[index].node_id, index);
		}
		const constraints: KeyDragBezierAlignmentConstraint[] = [];
		const unique_drag_key_ids = new Set(drag_key_ids);
		for (const key_id of unique_drag_key_ids) {
			const index = index_by_key_id.get(key_id);
			if (index === undefined || index <= 0 || index >= context.model.parsed_keys.length - 1) {
				continue;
			}
			const previous = context.model.parsed_keys[index - 1];
			const anchor = context.model.parsed_keys[index];
			const next = context.model.parsed_keys[index + 1];
			const left_segment = context.model.resolve_bezier_handle_segment(previous, anchor);
			const right_segment = context.model.resolve_bezier_handle_segment(anchor, next);
			if (!left_segment || !right_segment) {
				continue;
			}
			const in_vector_x = left_segment.in_curve_position - anchor.position;
			const in_vector_y = left_segment.in_curve_value - anchor.value;
			const out_vector_x = right_segment.out_curve_position - anchor.position;
			const out_vector_y = right_segment.out_curve_value - anchor.value;
			const in_length = Math.hypot(in_vector_x, in_vector_y);
			const out_length = Math.hypot(out_vector_x, out_vector_y);
			const denominator = in_length * out_length;
			if (denominator <= CURVE_EPSILON) {
				continue;
			}
			const cross = in_vector_x * out_vector_y - in_vector_y * out_vector_x;
			const dot = in_vector_x * out_vector_x + in_vector_y * out_vector_y;
			if (dot >= 0) {
				continue;
			}
			if (Math.abs(cross) / denominator > HANDLE_ALIGNMENT_COSINE_TOLERANCE) {
				continue;
			}
			constraints.push({
				anchor_key_id: anchor.node_id,
				previous_key_id: previous.node_id,
				next_key_id: next.node_id,
				previous_owner_key_id: left_segment.key_id,
				anchor_owner_key_id: right_segment.key_id,
				left_span: left_segment.span,
				right_span: right_segment.span,
				previous_out_vector_x: left_segment.out_curve_position - previous.position,
				previous_out_vector_y: left_segment.out_curve_value - previous.value,
				in_vector_x,
				in_vector_y,
				out_vector_x,
				out_vector_y,
				next_in_vector_x: right_segment.in_curve_position - next.position,
				next_in_vector_y: right_segment.in_curve_value - next.value
			});
		}
		return constraints;
	};
	return {
		clamp_curve_position_to_active_range,
		clamp_curve_value_to_active_range,
		clamp_curve_point_to_active_range,
		key_hit_threshold_px,
		curve_hit_threshold_px,
		bezier_handle_hit_threshold_px,
		HANDLE_ALIGNMENT_COSINE_TOLERANCE,
		KEY_NEIGHBOR_POSITION_GAP,
		build_key_drag_position_delta_constraint,
		clamp_key_drag_position_delta,
		build_key_drag_bezier_alignment_constraints
	};
}
