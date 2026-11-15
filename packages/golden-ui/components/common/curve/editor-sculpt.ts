import {
	KEY_NODE_TYPE,
	EASING_NODE_TYPE,
	DECL_EASING,
	DECL_KIND,
	DECL_OUT_POSITION,
	DECL_OUT_VALUE,
	DECL_IN_POSITION,
	DECL_IN_VALUE,
	CURVE_EPSILON,
	CURVE_SCULPT_SHIFT_FULL_STRENGTH_PX
} from './constants';
import {
	clamp,
	param_number_value,
	param_string_value,
	normalize_easing_kind,
	to_param_ref
} from './values';
import type { NodeId } from '../../../types';
import type {
	ParamNodeRef,
	DragPreview,
	BezierHandleKind,
	BezierHandleRef,
	BezierHandleSegment,
	BezierHandleControl,
	BezierEasingPreview,
	ActiveKeyDrag,
	ActiveBezierHandleDrag,
	CurveOppositeAlignment,
	ActiveCurveDrag
} from './types';
import type { CurveEditorDocument } from './editor-context';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorState } from './editor-state.svelte';
interface Context {
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly doc: CurveEditorDocument;
}
export function createCurveEditorSculpt(context: Context) {
	const bezier_preview_from_control = (control: BezierHandleControl): BezierEasingPreview => {
		const params = control.params;
		return {
			out_position: param_number_value(params[DECL_OUT_POSITION]?.value, 1 / 3),
			out_value: param_number_value(params[DECL_OUT_VALUE]?.value, 1 / 3),
			in_position: param_number_value(params[DECL_IN_POSITION]?.value, -1 / 3),
			in_value: param_number_value(params[DECL_IN_VALUE]?.value, -1 / 3)
		};
	};
	const clamp_bezier_handle_x = (control: BezierHandleControl, position: number): number =>
		clamp(position, control.segment_start_position, control.segment_end_position);
	const with_bezier_preview_handle = (
		control: BezierHandleControl,
		preview: BezierEasingPreview,
		kind: BezierHandleKind,
		handle_position: number,
		handle_value: number
	): BezierEasingPreview => {
		const span = Math.max(CURVE_EPSILON, control.span);
		const next = { ...preview };
		if (kind === 'out') {
			next.out_position = (handle_position - control.segment_start_position) / span;
			next.out_value = handle_value - control.anchor_value;
			return next;
		}
		next.in_position = (handle_position - control.segment_end_position) / span;
		next.in_value = handle_value - control.anchor_value;
		return next;
	};
	const ensure_bezier_preview_target = (
		targets: Map<NodeId, BezierEasingPreview>,
		key_id: NodeId
	): BezierEasingPreview | null => {
		const existing = targets.get(key_id);
		if (existing) {
			return existing;
		}
		const out_control = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id({ key_id, kind: 'out' })
		);
		const in_control = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id({ key_id, kind: 'in' })
		);
		let preview: BezierEasingPreview | null = null;
		if (out_control) {
			preview = bezier_preview_from_control(out_control);
		} else if (in_control) {
			preview = bezier_preview_from_control(in_control);
		} else {
			const params = bezier_param_refs_by_key_id(key_id);
			if (!params) {
				return null;
			}
			preview = {
				out_position: param_number_value(params[DECL_OUT_POSITION]?.value, 1 / 3),
				out_value: param_number_value(params[DECL_OUT_VALUE]?.value, 1 / 3),
				in_position: param_number_value(params[DECL_IN_POSITION]?.value, -1 / 3),
				in_value: param_number_value(params[DECL_IN_VALUE]?.value, -1 / 3)
			};
		}
		targets.set(key_id, preview);
		return preview;
	};
	const vectors_are_aligned_and_opposite = (
		ax: number,
		ay: number,
		bx: number,
		by: number
	): boolean => {
		const a_length = Math.hypot(ax, ay);
		const b_length = Math.hypot(bx, by);
		const denominator = a_length * b_length;
		if (denominator <= CURVE_EPSILON) {
			return false;
		}
		const dot = ax * bx + ay * by;
		if (dot >= 0) {
			return false;
		}
		const cross = ax * by - ay * bx;
		return Math.abs(cross) / denominator <= context.constraints.HANDLE_ALIGNMENT_COSINE_TOLERANCE;
	};
	const resolve_curve_drag_opposite_alignment = (
		anchor_key_id: NodeId,
		primary_ref: BezierHandleRef
	): CurveOppositeAlignment | null => {
		const primary = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id(primary_ref)
		);
		if (!primary) {
			return null;
		}
		const opposite_candidates =
			context.model.all_bezier_handle_refs_by_anchor_key_id.get(anchor_key_id) ?? [];
		let opposite_ref: BezierHandleRef | null = null;
		for (const candidate of opposite_candidates) {
			if (
				context.model.bezier_handle_control_id(candidate) ===
				context.model.bezier_handle_control_id(primary_ref)
			) {
				continue;
			}
			opposite_ref = candidate;
			break;
		}
		if (!opposite_ref) {
			return null;
		}
		const opposite = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id(opposite_ref)
		);
		if (!opposite) {
			return null;
		}
		const primary_dx = primary.handle_position - primary.anchor_position;
		const primary_dy = primary.handle_value - primary.anchor_value;
		const opposite_dx = opposite.handle_position - opposite.anchor_position;
		const opposite_dy = opposite.handle_value - opposite.anchor_value;
		if (!vectors_are_aligned_and_opposite(primary_dx, primary_dy, opposite_dx, opposite_dy)) {
			return null;
		}
		const opposite_length = Math.hypot(opposite_dx, opposite_dy);
		if (!(opposite_length > CURVE_EPSILON)) {
			return null;
		}
		return {
			anchor_key_id,
			opposite_ref,
			opposite_length
		};
	};
	const apply_curve_drag_opposite_alignment = (
		targets: Map<NodeId, BezierEasingPreview>,
		alignment: CurveOppositeAlignment,
		primary_handle_position: number,
		primary_handle_value: number,
		anchor_position: number,
		anchor_value: number
	): void => {
		const opposite_control = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id(alignment.opposite_ref)
		);
		if (!opposite_control || !(alignment.opposite_length > CURVE_EPSILON)) {
			return;
		}
		const primary_dx = primary_handle_position - anchor_position;
		const primary_dy = primary_handle_value - anchor_value;
		const primary_length = Math.hypot(primary_dx, primary_dy);
		if (primary_length <= CURVE_EPSILON) {
			return;
		}
		const opposite_direction_x = -primary_dx / primary_length;
		const opposite_direction_y = -primary_dy / primary_length;
		const projected = project_handle_vector_within_segment(
			opposite_control,
			anchor_position,
			anchor_value,
			opposite_direction_x * alignment.opposite_length,
			opposite_direction_y * alignment.opposite_length
		);
		const opposite_preview = ensure_bezier_preview_target(targets, opposite_control.ref.key_id);
		if (!opposite_preview) {
			return;
		}
		const updated_preview = with_bezier_preview_handle(
			opposite_control,
			opposite_preview,
			opposite_control.ref.kind,
			projected.position,
			projected.value
		);
		targets.set(opposite_control.ref.key_id, updated_preview);
	};
	const project_handle_vector_within_segment = (
		control: Pick<BezierHandleControl, 'segment_start_position' | 'segment_end_position'>,
		anchor_position: number,
		anchor_value: number,
		vector_x: number,
		vector_y: number
	): {
		position: number;
		value: number;
	} => {
		const target_position = anchor_position + vector_x;
		const target_value = anchor_value + vector_y;
		const clamped_position = clamp(
			target_position,
			control.segment_start_position,
			control.segment_end_position
		);
		if (Math.abs(clamped_position - target_position) <= CURVE_EPSILON) {
			return {
				position: clamped_position,
				value: target_value
			};
		}
		if (Math.abs(vector_x) <= CURVE_EPSILON) {
			return {
				position: clamped_position,
				value: target_value
			};
		}
		const scale = clamp((clamped_position - anchor_position) / vector_x, 0, 1);
		return {
			position: clamped_position,
			value: anchor_value + vector_y * scale
		};
	};
	const clamp_handle_position_with_value = (
		control: Pick<BezierHandleControl, 'segment_start_position' | 'segment_end_position'>,
		position: number,
		value: number
	): {
		position: number;
		value: number;
	} => ({
		position: clamp(position, control.segment_start_position, control.segment_end_position),
		value
	});
	const bezier_preview_from_absolute_handles = (
		segment: BezierHandleSegment,
		out_curve_position: number,
		out_curve_value: number,
		in_curve_position: number,
		in_curve_value: number
	): BezierEasingPreview => {
		const span = Math.max(CURVE_EPSILON, segment.span);
		const clamped_out_position = clamp(
			out_curve_position,
			segment.start_position,
			segment.end_position
		);
		const clamped_in_position = clamp(
			in_curve_position,
			segment.start_position,
			segment.end_position
		);
		return {
			out_position: (clamped_out_position - segment.start_position) / span,
			out_value: out_curve_value - segment.start_value,
			in_position: (clamped_in_position - segment.end_position) / span,
			in_value: in_curve_value - segment.end_value
		};
	};
	const update_curve_drag_preview = (
		drag: ActiveCurveDrag,
		curve_point: {
			position: number;
			value: number;
		},
		screen_x: number,
		absolute_mode: boolean,
		shift_mode: boolean
	): Map<NodeId, BezierEasingPreview> => {
		const segment = drag.segment;
		const targets = new Map<NodeId, BezierEasingPreview>();
		let out_curve_position: number;
		let out_curve_value: number;
		let in_curve_position: number;
		let in_curve_value: number;
		if (shift_mode) {
			const full_strength_px = Math.max(
				1,
				context.state.rem_base_px * CURVE_SCULPT_SHIFT_FULL_STRENGTH_PX
			);
			const horizontal_delta = screen_x - drag.start_screen_x;
			const strength = clamp(Math.abs(horizontal_delta) / full_strength_px, 0, 1);
			if (horizontal_delta >= 0) {
				targets.set(segment.key_id, {
					out_position: strength,
					out_value: 0,
					in_position: -strength,
					in_value: 0
				});
				return targets;
			}
			targets.set(segment.key_id, {
				out_position: 0,
				out_value: strength,
				in_position: 0,
				in_value: -strength
			});
			return targets;
		}
		if (absolute_mode) {
			const midpoint_vector = (
				anchor_position: number,
				anchor_value: number
			): {
				x: number;
				y: number;
			} => ({
				x: (curve_point.position - anchor_position) * 0.5,
				y: (curve_point.value - anchor_value) * 0.5
			});
			const out_midpoint = midpoint_vector(segment.start_position, segment.start_value);
			const out_target = clamp_handle_position_with_value(
				{
					segment_start_position: segment.start_position,
					segment_end_position: segment.end_position
				},
				segment.start_position + out_midpoint.x,
				segment.start_value + out_midpoint.y
			);
			out_curve_position = out_target.position;
			out_curve_value = out_target.value;
			const in_midpoint = midpoint_vector(segment.end_position, segment.end_value);
			const in_target = clamp_handle_position_with_value(
				{
					segment_start_position: segment.start_position,
					segment_end_position: segment.end_position
				},
				segment.end_position + in_midpoint.x,
				segment.end_value + in_midpoint.y
			);
			in_curve_position = in_target.position;
			in_curve_value = in_target.value;
		} else {
			const delta_position = curve_point.position - drag.start_curve_position;
			const delta_value = curve_point.value - drag.start_curve_value;
			const t = drag.normalized_t;
			const out_weight_basis = (1 - t) * (1 - t) * t;
			const in_weight_basis = (1 - t) * t * t;
			const sum = out_weight_basis + in_weight_basis;
			const out_weight = sum > CURVE_EPSILON ? out_weight_basis / sum : t <= 0.5 ? 1 : 0;
			const in_weight = sum > CURVE_EPSILON ? in_weight_basis / sum : t <= 0.5 ? 0 : 1;
			out_curve_position = drag.base_out_curve_position + delta_position * out_weight;
			out_curve_value = drag.base_out_curve_value + delta_value * out_weight;
			in_curve_position = drag.base_in_curve_position + delta_position * in_weight;
			in_curve_value = drag.base_in_curve_value + delta_value * in_weight;
		}
		const current_segment_preview = bezier_preview_from_absolute_handles(
			segment,
			out_curve_position,
			out_curve_value,
			in_curve_position,
			in_curve_value
		);
		targets.set(segment.key_id, current_segment_preview);
		if (drag.start_opposite_alignment) {
			apply_curve_drag_opposite_alignment(
				targets,
				drag.start_opposite_alignment,
				out_curve_position,
				out_curve_value,
				segment.start_position,
				segment.start_value
			);
		}
		if (drag.end_opposite_alignment) {
			apply_curve_drag_opposite_alignment(
				targets,
				drag.end_opposite_alignment,
				in_curve_position,
				in_curve_value,
				segment.end_position,
				segment.end_value
			);
		}
		return targets;
	};
	const update_key_drag_bezier_alignment_targets = (
		drag: ActiveKeyDrag,
		commit_targets: Map<NodeId, DragPreview>
	): Map<NodeId, BezierEasingPreview> => {
		const targets = new Map<NodeId, BezierEasingPreview>();
		if (drag.bezier_alignment_constraints.length === 0) {
			return targets;
		}
		const curve_point_for_key = (
			key_id: NodeId
		): {
			position: number;
			value: number;
		} | null => {
			const translated = commit_targets.get(key_id);
			if (translated) {
				return translated;
			}
			const key = context.model.parsed_key_by_id.get(key_id);
			if (!key) {
				return null;
			}
			return { position: key.position, value: key.value };
		};
		for (const constraint of drag.bezier_alignment_constraints) {
			const previous_point = curve_point_for_key(constraint.previous_key_id);
			const anchor_point = curve_point_for_key(constraint.anchor_key_id);
			const next_point = curve_point_for_key(constraint.next_key_id);
			if (!previous_point || !anchor_point || !next_point) {
				continue;
			}
			const left_span = anchor_point.position - previous_point.position;
			const right_span = next_point.position - anchor_point.position;
			if (
				!(constraint.left_span > CURVE_EPSILON) ||
				!(constraint.right_span > CURVE_EPSILON) ||
				!(left_span > CURVE_EPSILON) ||
				!(right_span > CURVE_EPSILON)
			) {
				continue;
			}
			const in_scale = left_span / constraint.left_span;
			const out_scale = right_span / constraint.right_span;
			if (!Number.isFinite(in_scale) || !Number.isFinite(out_scale)) {
				continue;
			}
			const scaled_in_vector_x = constraint.in_vector_x * in_scale;
			const scaled_in_vector_y = constraint.in_vector_y * in_scale;
			const scaled_out_vector_x = constraint.out_vector_x * out_scale;
			const scaled_out_vector_y = constraint.out_vector_y * out_scale;
			const previous_owner_preview = ensure_bezier_preview_target(
				targets,
				constraint.previous_owner_key_id
			);
			if (previous_owner_preview) {
				previous_owner_preview.out_position =
					(constraint.previous_out_vector_x * in_scale) / left_span;
				previous_owner_preview.out_value = constraint.previous_out_vector_y * in_scale;
				previous_owner_preview.in_position = scaled_in_vector_x / left_span;
				previous_owner_preview.in_value = scaled_in_vector_y;
			}
			const anchor_owner_preview = ensure_bezier_preview_target(
				targets,
				constraint.anchor_owner_key_id
			);
			if (anchor_owner_preview) {
				anchor_owner_preview.out_position = scaled_out_vector_x / right_span;
				anchor_owner_preview.out_value = scaled_out_vector_y;
				anchor_owner_preview.in_position = (constraint.next_in_vector_x * out_scale) / right_span;
				anchor_owner_preview.in_value = constraint.next_in_vector_y * out_scale;
			}
		}
		return targets;
	};
	const bezier_param_refs_by_key_id = (key_id: NodeId): Record<string, ParamNodeRef> | null => {
		const nodes_by_id = context.doc.graphNodesById;
		if (!nodes_by_id) {
			return null;
		}
		const key_node = nodes_by_id.get(key_id);
		if (!key_node || key_node.node_type !== KEY_NODE_TYPE) {
			return null;
		}
		for (const key_child_id of key_node.children) {
			const key_child = nodes_by_id.get(key_child_id);
			if (
				!key_child ||
				key_child.decl_id !== DECL_EASING ||
				key_child.node_type !== EASING_NODE_TYPE
			) {
				continue;
			}
			const easing_params: Record<string, ParamNodeRef> = {};
			for (const easing_child_id of key_child.children) {
				const easing_child = nodes_by_id.get(easing_child_id);
				const param_ref = to_param_ref(easing_child);
				if (!easing_child || !param_ref) {
					continue;
				}
				easing_params[easing_child.decl_id] = param_ref;
			}
			const easing_kind = normalize_easing_kind(
				param_string_value(easing_params[DECL_KIND]?.value, 'linear')
			);
			if (easing_kind !== 'bezier') {
				return null;
			}
			return easing_params;
		}
		return null;
	};
	const bezier_control_for_anchor_kind = (
		anchor_key_id: NodeId,
		kind: BezierHandleKind
	): BezierHandleControl | null => {
		const refs = context.model.all_bezier_handle_refs_by_anchor_key_id.get(anchor_key_id) ?? [];
		for (const ref of refs) {
			if (ref.kind !== kind) {
				continue;
			}
			const control = context.model.all_bezier_handle_control_by_id.get(
				context.model.bezier_handle_control_id(ref)
			);
			if (control) {
				return control;
			}
		}
		return null;
	};
	const update_bezier_handle_drag_preview = (
		drag: ActiveBezierHandleDrag,
		curve_point: {
			position: number;
			value: number;
		},
		individual_mode: boolean
	): Map<NodeId, BezierEasingPreview> => {
		const targets = new Map<NodeId, BezierEasingPreview>();
		const control = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id(drag.ref)
		);
		if (!control) {
			return targets;
		}
		const active_base_preview = bezier_preview_from_control(control);
		const dragged_x = clamp_bezier_handle_x(control, curve_point.position);
		const active_preview = with_bezier_preview_handle(
			control,
			active_base_preview,
			drag.ref.kind,
			dragged_x,
			curve_point.value
		);
		targets.set(control.ref.key_id, active_preview);
		if (individual_mode || !drag.opposite_ref || drag.opposite_length <= CURVE_EPSILON) {
			return targets;
		}
		const opposite = context.model.all_bezier_handle_control_by_id.get(
			context.model.bezier_handle_control_id(drag.opposite_ref)
		);
		if (!opposite) {
			return targets;
		}
		const vector_x = dragged_x - control.anchor_position;
		const vector_y = curve_point.value - control.anchor_value;
		const vector_length = Math.hypot(vector_x, vector_y);
		if (vector_length <= CURVE_EPSILON) {
			return targets;
		}
		const opposite_base_preview = bezier_preview_from_control(opposite);
		const opposite_direction_x = -vector_x / vector_length;
		const opposite_direction_y = -vector_y / vector_length;
		const projected_opposite = project_handle_vector_within_segment(
			opposite,
			opposite.anchor_position,
			opposite.anchor_value,
			opposite_direction_x * drag.opposite_length,
			opposite_direction_y * drag.opposite_length
		);
		const opposite_preview = with_bezier_preview_handle(
			opposite,
			opposite_base_preview,
			opposite.ref.kind,
			projected_opposite.position,
			projected_opposite.value
		);
		targets.set(opposite.ref.key_id, opposite_preview);
		return targets;
	};
	return {
		bezier_preview_from_control,
		clamp_bezier_handle_x,
		with_bezier_preview_handle,
		ensure_bezier_preview_target,
		vectors_are_aligned_and_opposite,
		resolve_curve_drag_opposite_alignment,
		apply_curve_drag_opposite_alignment,
		project_handle_vector_within_segment,
		clamp_handle_position_with_value,
		bezier_preview_from_absolute_handles,
		update_curve_drag_preview,
		update_key_drag_bezier_alignment_targets,
		bezier_param_refs_by_key_id,
		bezier_control_for_anchor_kind,
		update_bezier_handle_drag_preview
	};
}
