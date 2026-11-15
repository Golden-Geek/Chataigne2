import {
	DECL_KIND,
	DECL_OUT_POSITION,
	DECL_OUT_VALUE,
	DECL_IN_POSITION,
	DECL_IN_VALUE,
	CURVE_EPSILON
} from './constants';
import { param_number_value, param_string_value, normalize_easing_kind } from './values';
import type { NodeId, ParamValue } from '../../../types';
import type { DragPreview, BezierEasingPreview, CurveCreatePlan } from './types';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorState } from './editor-state.svelte';
interface Context {
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly state: ReturnType<typeof createCurveEditorState>;
}
export function createCurveEditorPreviews(context: Context) {
	const set_drag_preview = (key_id: NodeId, position: number, value: number): void => {
		const clamped = context.constraints.clamp_curve_point_to_active_range(position, value);
		const existing = context.state.drag_preview_by_key_id.get(key_id);
		if (
			existing &&
			Math.abs(existing.position - clamped.position) <= CURVE_EPSILON &&
			Math.abs(existing.value - clamped.value) <= CURVE_EPSILON
		) {
			return;
		}
		const next = new Map(context.state.drag_preview_by_key_id);
		next.set(key_id, { position: clamped.position, value: clamped.value });
		context.state.drag_preview_by_key_id = next;
	};
	const clear_drag_preview = (key_id: NodeId): void => {
		if (!context.state.drag_preview_by_key_id.has(key_id)) {
			return;
		}
		const next = new Map(context.state.drag_preview_by_key_id);
		next.delete(key_id);
		context.state.drag_preview_by_key_id = next;
	};
	const set_easing_drag_preview = (key_id: NodeId, preview: BezierEasingPreview): void => {
		const existing = context.state.easing_drag_preview_by_key_id.get(key_id);
		if (
			existing &&
			Math.abs(existing.out_position - preview.out_position) <= CURVE_EPSILON &&
			Math.abs(existing.out_value - preview.out_value) <= CURVE_EPSILON &&
			Math.abs(existing.in_position - preview.in_position) <= CURVE_EPSILON &&
			Math.abs(existing.in_value - preview.in_value) <= CURVE_EPSILON
		) {
			return;
		}
		const next = new Map(context.state.easing_drag_preview_by_key_id);
		next.set(key_id, preview);
		context.state.easing_drag_preview_by_key_id = next;
	};
	const clear_easing_drag_preview = (key_id: NodeId): void => {
		if (!context.state.easing_drag_preview_by_key_id.has(key_id)) {
			return;
		}
		const next = new Map(context.state.easing_drag_preview_by_key_id);
		next.delete(key_id);
		context.state.easing_drag_preview_by_key_id = next;
	};
	const clear_drag_previews_if_unchanged = (snapshots: Map<NodeId, DragPreview>): void => {
		if (snapshots.size === 0 || context.state.drag_preview_by_key_id.size === 0) {
			return;
		}
		const current = context.state.drag_preview_by_key_id;
		const next = new Map(current);
		let changed = false;
		for (const [key_id, snapshot] of snapshots.entries()) {
			if (current.get(key_id) !== snapshot) {
				continue;
			}
			if (next.delete(key_id)) {
				changed = true;
			}
		}
		if (changed) {
			context.state.drag_preview_by_key_id = next;
		}
	};
	const clear_easing_drag_previews_if_unchanged = (
		snapshots: Map<NodeId, BezierEasingPreview>
	): void => {
		if (snapshots.size === 0 || context.state.easing_drag_preview_by_key_id.size === 0) {
			return;
		}
		const current = context.state.easing_drag_preview_by_key_id;
		const next = new Map(current);
		let changed = false;
		for (const [key_id, snapshot] of snapshots.entries()) {
			if (current.get(key_id) !== snapshot) {
				continue;
			}
			if (next.delete(key_id)) {
				changed = true;
			}
		}
		if (changed) {
			context.state.easing_drag_preview_by_key_id = next;
		}
	};
	const bezier_preview_from_param_values = (
		values: Record<string, ParamValue> | null | undefined
	): BezierEasingPreview | null => {
		if (!values || Object.keys(values).length === 0) {
			return null;
		}
		const next_kind = values[DECL_KIND];
		if (next_kind && normalize_easing_kind(param_string_value(next_kind, 'linear')) !== 'bezier') {
			return null;
		}
		if (
			!next_kind &&
			!values[DECL_OUT_POSITION] &&
			!values[DECL_OUT_VALUE] &&
			!values[DECL_IN_POSITION] &&
			!values[DECL_IN_VALUE]
		) {
			return null;
		}
		return {
			out_position: param_number_value(values[DECL_OUT_POSITION], 1 / 3),
			out_value: param_number_value(values[DECL_OUT_VALUE], 1 / 3),
			in_position: param_number_value(values[DECL_IN_POSITION], -1 / 3),
			in_value: param_number_value(values[DECL_IN_VALUE], -1 / 3)
		};
	};
	const sync_pending_create_previews = (pending: CurveCreatePlan, created_key_id: NodeId): void => {
		set_drag_preview(created_key_id, pending.position, pending.value);
		const sync_bezier_preview = (
			key_id: NodeId,
			values: Record<string, ParamValue> | null | undefined
		): void => {
			const preview = bezier_preview_from_param_values(values);
			if (!preview) {
				return;
			}
			set_easing_drag_preview(key_id, preview);
		};
		if (pending.split_easing) {
			sync_bezier_preview(
				pending.split_easing.source_start_key_id,
				pending.split_easing.left_param_values
			);
			sync_bezier_preview(created_key_id, pending.split_easing.right_param_values);
			for (const update of pending.split_easing.preserved_aligned_handle_updates ?? []) {
				sync_bezier_preview(update.key_id, update.values);
			}
		} else {
			sync_bezier_preview(created_key_id, pending.easing_values);
		}
		for (const update of pending.existing_easing_updates) {
			sync_bezier_preview(update.key_id, update.values);
		}
	};
	return {
		set_drag_preview,
		clear_drag_preview,
		set_easing_drag_preview,
		clear_easing_drag_preview,
		clear_drag_previews_if_unchanged,
		clear_easing_drag_previews_if_unchanged,
		bezier_preview_from_param_values,
		sync_pending_create_previews
	};
}
