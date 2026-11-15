import { curveGeometryParameters } from './admission';
import {
	CURVE_EPSILON,
	CANVAS_DOUBLE_CLICK_WINDOW_MS,
	CANVAS_DOUBLE_CLICK_RADIUS_PX
} from './constants';
import { clamp } from './values';
import type { NodeId } from '../../../types';
import type { DragPreview, BezierHandleRef, CanvasPrimaryDownSample, SelectionMode } from './types';
import type { CurveEditorServices, CurveEditorBindings } from './editor-context';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorHits } from './editor-hits';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorSelection } from './editor-selection.svelte';
import type { createCurveEditorCommands } from './editor-commands.svelte';
import type { createCurveEditorCamera } from './editor-camera';
import type { createCurveEditorDrawing } from './editor-drawing';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorSculpt } from './editor-sculpt';
import type { createCurveEditorPreviews } from './editor-previews';
interface Context {
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly hits: ReturnType<typeof createCurveEditorHits>;
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly selection: ReturnType<typeof createCurveEditorSelection>;
	readonly commands: ReturnType<typeof createCurveEditorCommands>;
	readonly services: CurveEditorServices;
	readonly camera: ReturnType<typeof createCurveEditorCamera>;
	readonly drawing: ReturnType<typeof createCurveEditorDrawing>;
	readonly bindings: CurveEditorBindings;
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly sculpt: ReturnType<typeof createCurveEditorSculpt>;
	readonly previews: ReturnType<typeof createCurveEditorPreviews>;
}
export function createCurveEditorPointer(context: Context) {
	const shortcut_modifier_active = (event: MouseEvent | PointerEvent): boolean =>
		event.ctrlKey || event.metaKey;
	const matches_canvas_double_activation = (
		previous: CanvasPrimaryDownSample | null,
		event: PointerEvent,
		screen_x: number,
		screen_y: number
	): boolean => {
		if (!previous) {
			return false;
		}
		if (event.timeStamp - previous.time_stamp > CANVAS_DOUBLE_CLICK_WINDOW_MS) {
			return false;
		}
		return (
			Math.hypot(screen_x - previous.screen_x, screen_y - previous.screen_y) <=
			CANVAS_DOUBLE_CLICK_RADIUS_PX
		);
	};
	const blur_active_editable_element = (): void => {
		if (typeof document === 'undefined') {
			return;
		}
		const active = document.activeElement;
		if (!(active instanceof HTMLElement)) {
			return;
		}
		if (active.isContentEditable) {
			active.blur();
			return;
		}
		const tag = active.tagName;
		if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
			active.blur();
		}
	};
	const on_canvas_context_menu = (event: MouseEvent): void => {
		if (!context.state.interaction_transform || !context.state.canvas_element) {
			return;
		}
		const point = context.hits.canvas_local_point(event);
		if (!point) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		blur_active_editable_element();
		context.state.canvas_element.focus();
		context.state.is_canvas_focused = true;
		context.state.hover_curve_position = context.hits.screen_to_curve_point(point.x, point.y);
		const key_id = context.hits.nearest_key(
			point.x,
			point.y,
			context.constraints.key_hit_threshold_px()
		);
		if (key_id !== null) {
			context.selection.set_single_selected_key(key_id);
			context.state.hover_key_id = key_id;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			context.commands.open_curve_context_menu(
				{ kind: 'key', key_id },
				event.clientX,
				event.clientY
			);
			return;
		}
		const curve_hit = context.hits.nearest_curve_hit(
			point.x,
			point.y,
			context.constraints.curve_hit_threshold_px()
		);
		if (curve_hit) {
			context.selection.set_single_selected_curve_owner_key(curve_hit.owner_key_id);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = curve_hit.owner_key_id;
			context.state.hover_bezier_handle = null;
			context.commands.open_curve_context_menu(
				{ kind: 'curve', key_id: curve_hit.owner_key_id },
				event.clientX,
				event.clientY
			);
			return;
		}
		context.state.hover_key_id = null;
		context.state.hover_curve_owner_key_id = null;
		context.state.hover_bezier_handle = null;
		context.commands.open_curve_context_menu({ kind: 'all' }, event.clientX, event.clientY);
	};
	const on_canvas_pointer_down = (event: PointerEvent): void => {
		if (
			context.services.input.active ||
			!context.state.interaction_transform ||
			!context.state.canvas_element
		) {
			return;
		}
		const point = context.hits.canvas_local_point(event);
		if (!point) {
			return;
		}
		blur_active_editable_element();
		context.state.canvas_element.focus();
		context.state.is_canvas_focused = true;
		context.state.hover_curve_position = context.hits.screen_to_curve_point(point.x, point.y);
		if (event.button === 1) {
			const start_bounds = context.camera.current_view_bounds();
			if (!start_bounds) {
				return;
			}
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.active_canvas_pan = {
				pointer_id: event.pointerId,
				start_screen_x: point.x,
				start_screen_y: point.y,
				start_bounds
			};
			context.camera.apply_view_bounds(start_bounds);
			context.state.canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}
		if (event.button !== 0) {
			return;
		}
		const curve_point = context.state.hover_curve_position;
		const selection = context.hits.nearest_key(
			point.x,
			point.y,
			context.constraints.key_hit_threshold_px()
		);
		const bezier_handle = context.hits.nearest_bezier_handle(
			point.x,
			point.y,
			context.constraints.bezier_handle_hit_threshold_px()
		);
		const curve_hit = context.hits.nearest_curve_hit(
			point.x,
			point.y,
			context.constraints.curve_hit_threshold_px()
		);
		const is_double_activation = matches_canvas_double_activation(
			context.state.last_canvas_primary_down,
			event,
			point.x,
			point.y
		);
		context.state.last_canvas_primary_down = {
			time_stamp: event.timeStamp,
			screen_x: point.x,
			screen_y: point.y
		};
		if (is_double_activation) {
			if (
				context.commands.handle_canvas_double_activation(
					selection,
					bezier_handle,
					curve_hit,
					curve_point
				)
			) {
				context.state.last_canvas_primary_down = null;
				event.preventDefault();
				return;
			}
		}
		if (
			shortcut_modifier_active(event) &&
			context.commands.handle_canvas_shortcut_click(
				selection,
				curve_hit,
				curve_point,
				event.shiftKey
			)
		) {
			event.preventDefault();
			return;
		}
		if (event.ctrlKey && event.shiftKey && selection === null && !bezier_handle && !curve_hit) {
			if (!curve_point) {
				return;
			}
			context.state.active_curve_draw = context.drawing.record_curve_draw_point(
				{
					pointer_id: event.pointerId,
					points: [],
					path_buckets: []
				},
				point.x,
				curve_point
			);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			context.state.active_box_selection = null;
			context.state.canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}
		const additive_selection = event.shiftKey;
		if (selection !== null) {
			if (additive_selection) {
				context.selection.add_selected_key(selection);
				context.bindings.selected_key_id = selection;
				context.state.hover_curve_owner_key_id = null;
				event.preventDefault();
				return;
			}
			if (!context.selection.has_selected_key(selection)) {
				context.selection.set_single_selected_key(selection);
			} else {
				context.bindings.selected_key_id = selection;
			}
			const drag_key_ids = context.selection.has_selected_key(selection)
				? context.bindings.selected_key_ids
				: [selection];
			const drag_origins: Array<{
				key_id: NodeId;
				position: number;
				value: number;
			}> = [];
			for (const key_id of drag_key_ids) {
				const key = context.model.parsed_key_by_id.get(key_id);
				if (!key?.position_param || !key.value_param) {
					continue;
				}
				drag_origins.push({
					key_id,
					position: key.position,
					value: key.value
				});
			}
			const anchor_origin =
				drag_origins.find((entry) => entry.key_id === selection) ?? drag_origins[0] ?? null;
			if (!anchor_origin) {
				return;
			}
			const position_delta_constraint = context.bindings.constrainKeysToNeighbors
				? context.constraints.build_key_drag_position_delta_constraint(drag_origins)
				: null;
			const bezier_alignment_constraints =
				context.constraints.build_key_drag_bezier_alignment_constraints(drag_key_ids);
			const parameters = curveGeometryParameters(
				drag_key_ids.map((id) => context.model.parsed_key_by_id.get(id)),
				bezier_alignment_constraints.flatMap((constraint) => [
					context.sculpt.bezier_param_refs_by_key_id(constraint.previous_owner_key_id),
					context.sculpt.bezier_param_refs_by_key_id(constraint.anchor_owner_key_id)
				])
			);
			if (
				!parameters ||
				!context.services.editGesture.begin('Move Curve Key', 'curve-key-drag', parameters)
			)
				return;
			context.state.active_drag = {
				pointer_id: event.pointerId,
				anchor_key_id: anchor_origin.key_id,
				anchor_start_position: anchor_origin.position,
				anchor_start_value: anchor_origin.value,
				origins: drag_origins,
				position_delta_constraint,
				bezier_alignment_constraints,
				touched_bezier_key_ids: new Set<NodeId>()
			};
			context.state.canvas_element.setPointerCapture(event.pointerId);
			context.state.hover_curve_owner_key_id = null;
			event.preventDefault();
			return;
		}
		if (bezier_handle) {
			const control = context.model.all_bezier_handle_control_by_id.get(
				context.model.bezier_handle_control_id(bezier_handle)
			);
			if (!control) {
				return;
			}
			context.state.hover_bezier_handle = bezier_handle;
			context.state.hover_key_id = control.anchor_key_id;
			context.state.hover_curve_owner_key_id = null;
			const opposite_candidates =
				context.model.all_bezier_handle_refs_by_anchor_key_id.get(control.anchor_key_id) ?? [];
			let opposite_ref: BezierHandleRef | null = null;
			for (const candidate of opposite_candidates) {
				if (
					context.model.bezier_handle_control_id(candidate) ===
					context.model.bezier_handle_control_id(bezier_handle)
				) {
					continue;
				}
				opposite_ref = candidate;
				break;
			}
			let opposite_length = 0;
			if (opposite_ref) {
				const opposite_control = context.model.all_bezier_handle_control_by_id.get(
					context.model.bezier_handle_control_id(opposite_ref)
				);
				if (opposite_control) {
					const opposite_dx = opposite_control.handle_position - opposite_control.anchor_position;
					const opposite_dy = opposite_control.handle_value - opposite_control.anchor_value;
					opposite_length = Math.hypot(opposite_dx, opposite_dy);
				}
			}
			const parameters = curveGeometryParameters(
				[],
				[
					control.params,
					...(opposite_ref ? [context.sculpt.bezier_param_refs_by_key_id(opposite_ref.key_id)] : [])
				]
			);
			if (
				!parameters ||
				!context.services.editGesture.begin(
					'Adjust Curve Bezier Handle',
					'curve-bezier-drag',
					parameters
				)
			)
				return;
			context.state.active_bezier_handle_drag = {
				pointer_id: event.pointerId,
				ref: bezier_handle,
				opposite_ref,
				opposite_length,
				touched_key_ids: new Set<NodeId>([bezier_handle.key_id])
			};
			context.state.canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}
		if (curve_hit) {
			const curve_selection = curve_hit.owner_key_id;
			if (additive_selection) {
				context.selection.add_selected_curve_owner_key(curve_selection);
			} else if (
				!context.selection.has_selected_curve_owner_key(curve_selection) ||
				context.bindings.selected_curve_owner_key_ids.length !== 1
			) {
				context.selection.set_single_selected_curve_owner_key(curve_selection);
			}
			const segment = context.model.bezier_segment_by_owner_key_id.get(curve_selection);
			if (!segment) {
				event.preventDefault();
				return;
			}
			const normalized_t = clamp(
				(curve_hit.curve_position - segment.start_position) / Math.max(CURVE_EPSILON, segment.span),
				0,
				1
			);
			const out_length = Math.hypot(
				segment.out_curve_position - segment.start_position,
				segment.out_curve_value - segment.start_value
			);
			const in_length = Math.hypot(
				segment.in_curve_position - segment.end_position,
				segment.in_curve_value - segment.end_value
			);
			const start_opposite_alignment = context.sculpt.resolve_curve_drag_opposite_alignment(
				segment.start_key_id,
				{
					key_id: segment.key_id,
					kind: 'out'
				}
			);
			const end_opposite_alignment = context.sculpt.resolve_curve_drag_opposite_alignment(
				segment.end_key_id,
				{
					key_id: segment.key_id,
					kind: 'in'
				}
			);
			const parameters = curveGeometryParameters(
				[],
				[
					segment.params,
					...[start_opposite_alignment, end_opposite_alignment].flatMap((alignment) =>
						alignment
							? [context.sculpt.bezier_param_refs_by_key_id(alignment.opposite_ref.key_id)]
							: []
					)
				]
			);
			if (
				!parameters ||
				!context.services.editGesture.begin(
					'Sculpt Curve Segment',
					'curve-segment-drag',
					parameters
				)
			)
				return;
			context.state.active_curve_drag = {
				pointer_id: event.pointerId,
				owner_key_id: curve_selection,
				start_screen_x: point.x,
				start_curve_position: curve_hit.curve_position,
				start_curve_value: curve_hit.curve_value,
				segment,
				normalized_t,
				base_out_curve_position: segment.out_curve_position,
				base_out_curve_value: segment.out_curve_value,
				base_in_curve_position: segment.in_curve_position,
				base_in_curve_value: segment.in_curve_value,
				out_length,
				in_length,
				start_opposite_alignment,
				end_opposite_alignment,
				touched_key_ids: new Set<NodeId>([curve_selection])
			};
			context.state.canvas_element.setPointerCapture(event.pointerId);
			context.state.hover_curve_owner_key_id = curve_selection;
			context.state.hover_key_id = null;
			event.preventDefault();
			return;
		}
		const mode: SelectionMode = additive_selection ? 'add' : 'replace';
		if (!event.shiftKey && !event.altKey && !event.ctrlKey && !event.metaKey) {
			context.selection.clear_canvas_selection();
		}
		context.state.active_box_selection = {
			pointer_id: event.pointerId,
			start_screen_x: point.x,
			start_screen_y: point.y,
			current_screen_x: point.x,
			current_screen_y: point.y,
			mode
		};
		context.state.hover_key_id = null;
		context.state.hover_curve_owner_key_id = null;
		context.state.hover_bezier_handle = null;
		context.state.canvas_element.setPointerCapture(event.pointerId);
		event.preventDefault();
	};
	const on_canvas_pointer_move = (event: PointerEvent): void => {
		const point = context.hits.canvas_local_point(event);
		if (!point) {
			return;
		}
		context.state.hover_curve_position = context.hits.screen_to_curve_point(point.x, point.y);
		if (
			context.state.active_curve_draw &&
			context.state.active_curve_draw.pointer_id === event.pointerId
		) {
			const curve_point = context.state.hover_curve_position;
			if (!curve_point) {
				return;
			}
			context.state.active_curve_draw = context.drawing.record_curve_draw_point(
				context.state.active_curve_draw,
				point.x,
				curve_point
			);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			return;
		}
		if (
			context.state.active_canvas_pan &&
			context.state.active_canvas_pan.pointer_id === event.pointerId &&
			context.state.interaction_transform
		) {
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			const delta_x = point.x - context.state.active_canvas_pan.start_screen_x;
			const delta_y = point.y - context.state.active_canvas_pan.start_screen_y;
			const start = context.state.active_canvas_pan.start_bounds;
			const units_per_px_x =
				(start.x_max - start.x_min) / Math.max(1, context.state.interaction_transform.plot_width);
			const units_per_px_y =
				(start.y_max - start.y_min) / Math.max(1, context.state.interaction_transform.plot_height);
			context.camera.apply_view_bounds({
				x_min: start.x_min - delta_x * units_per_px_x,
				x_max: start.x_max - delta_x * units_per_px_x,
				y_min: start.y_min + delta_y * units_per_px_y,
				y_max: start.y_max + delta_y * units_per_px_y
			});
			return;
		}
		if (
			context.state.active_box_selection &&
			context.state.active_box_selection.pointer_id === event.pointerId
		) {
			context.state.active_box_selection = {
				...context.state.active_box_selection,
				current_screen_x: point.x,
				current_screen_y: point.y
			};
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			return;
		}
		if (
			context.state.active_bezier_handle_drag &&
			context.state.active_bezier_handle_drag.pointer_id === event.pointerId
		) {
			const curve_point = context.hits.screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			const preview_updates = context.sculpt.update_bezier_handle_drag_preview(
				context.state.active_bezier_handle_drag,
				curve_point,
				event.altKey
			);
			if (preview_updates.size === 0) {
				return;
			}
			for (const [key_id, preview] of preview_updates.entries()) {
				context.previews.set_easing_drag_preview(key_id, preview);
				context.state.active_bezier_handle_drag.touched_key_ids.add(key_id);
			}
			context.services.editGesture.queueBezier(preview_updates);
			context.state.hover_bezier_handle = context.state.active_bezier_handle_drag.ref;
			const active_control = context.model.all_bezier_handle_control_by_id.get(
				context.model.bezier_handle_control_id(context.state.active_bezier_handle_drag.ref)
			);
			if (active_control) {
				context.state.hover_key_id = active_control.anchor_key_id;
			}
			context.state.hover_curve_owner_key_id = null;
			return;
		}
		if (
			context.state.active_curve_drag &&
			context.state.active_curve_drag.pointer_id === event.pointerId
		) {
			const curve_point = context.hits.screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			context.state.hover_bezier_handle = null;
			const preview_updates = context.sculpt.update_curve_drag_preview(
				context.state.active_curve_drag,
				curve_point,
				point.x,
				event.altKey,
				event.shiftKey
			);
			if (preview_updates.size === 0) {
				return;
			}
			const updated_key_ids = new Set<NodeId>(preview_updates.keys());
			for (const key_id of [...context.state.active_curve_drag.touched_key_ids]) {
				if (updated_key_ids.has(key_id)) {
					continue;
				}
				context.previews.clear_easing_drag_preview(key_id);
				context.state.active_curve_drag.touched_key_ids.delete(key_id);
			}
			for (const [key_id, preview] of preview_updates.entries()) {
				context.previews.set_easing_drag_preview(key_id, preview);
				context.state.active_curve_drag.touched_key_ids.add(key_id);
			}
			context.services.editGesture.queueBezier(preview_updates);
			context.state.hover_key_id = null;
			context.state.hover_curve_owner_key_id = context.state.active_curve_drag.owner_key_id;
			return;
		}
		if (context.state.active_drag && context.state.active_drag.pointer_id === event.pointerId) {
			const curve_point = context.hits.screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			context.state.hover_bezier_handle = null;
			const raw_delta_position =
				curve_point.position - context.state.active_drag.anchor_start_position;
			const delta_position = context.constraints.clamp_key_drag_position_delta(
				context.state.active_drag,
				raw_delta_position
			);
			const delta_value = curve_point.value - context.state.active_drag.anchor_start_value;
			const commit_targets = new Map<NodeId, DragPreview>();
			for (const origin of context.state.active_drag.origins) {
				const translated = context.constraints.clamp_curve_point_to_active_range(
					origin.position + delta_position,
					origin.value + delta_value
				);
				context.previews.set_drag_preview(origin.key_id, translated.position, translated.value);
				commit_targets.set(origin.key_id, translated);
			}
			context.services.editGesture.queueKeys(commit_targets);
			const bezier_alignment_updates = context.sculpt.update_key_drag_bezier_alignment_targets(
				context.state.active_drag,
				commit_targets
			);
			const updated_key_ids = new Set<NodeId>(bezier_alignment_updates.keys());
			for (const key_id of [...context.state.active_drag.touched_bezier_key_ids]) {
				if (updated_key_ids.has(key_id)) {
					continue;
				}
				context.previews.clear_easing_drag_preview(key_id);
				context.state.active_drag.touched_bezier_key_ids.delete(key_id);
			}
			if (bezier_alignment_updates.size > 0) {
				for (const [key_id, preview] of bezier_alignment_updates.entries()) {
					context.previews.set_easing_drag_preview(key_id, preview);
					context.state.active_drag.touched_bezier_key_ids.add(key_id);
				}
				context.services.editGesture.queueBezier(bezier_alignment_updates);
			}
			context.state.hover_curve_owner_key_id = null;
			return;
		}
		const hovered_key = context.hits.nearest_key(
			point.x,
			point.y,
			context.constraints.key_hit_threshold_px()
		);
		if (hovered_key !== null) {
			context.state.hover_key_id = hovered_key;
			context.state.hover_curve_owner_key_id = null;
			context.state.hover_bezier_handle = null;
			return;
		}
		const hovered_bezier_handle = context.hits.nearest_bezier_handle(
			point.x,
			point.y,
			context.constraints.bezier_handle_hit_threshold_px()
		);
		context.state.hover_bezier_handle = hovered_bezier_handle;
		if (hovered_bezier_handle) {
			const hovered_control = context.model.all_bezier_handle_control_by_id.get(
				context.model.bezier_handle_control_id(hovered_bezier_handle)
			);
			context.state.hover_key_id = hovered_control?.anchor_key_id ?? hovered_bezier_handle.key_id;
			context.state.hover_curve_owner_key_id = null;
			return;
		}
		context.state.hover_key_id = null;
		context.state.hover_curve_owner_key_id = context.hits.nearest_curve_owner_key(
			point.x,
			point.y,
			context.constraints.curve_hit_threshold_px()
		);
	};
	const on_canvas_pointer_up = (event: PointerEvent): void =>
		context.services.input.finish(event.pointerId);
	const on_canvas_pointer_leave = (): void => {
		context.state.hover_key_id = null;
		context.state.hover_curve_owner_key_id = null;
		context.state.hover_bezier_handle = null;
		context.state.hover_curve_position = null;
		context.state.last_canvas_primary_down = null;
	};
	const on_canvas_focus = (): void => {
		context.state.is_canvas_focused = true;
	};
	const on_canvas_blur = (): void => {
		context.state.is_canvas_focused = false;
		context.state.last_canvas_primary_down = null;
	};
	const on_canvas_wheel = (event: WheelEvent): void => {
		if (!context.state.interaction_transform) {
			return;
		}
		const point = context.hits.canvas_local_point(event);
		if (!point) {
			return;
		}
		const delta = context.camera.normalize_wheel_delta(event);
		const should_pan =
			event.shiftKey || event.altKey || Math.abs(delta.x) > Math.abs(delta.y) * 0.65;
		if (should_pan) {
			let pan_x = delta.x;
			let pan_y = delta.y;
			if (event.shiftKey) {
				if (Math.abs(pan_x) < Math.abs(delta.y)) {
					pan_x += delta.y;
				}
				pan_y = 0;
			}
			if (event.altKey) {
				pan_x = 0;
			}
			context.camera.pan_view_by_pixels(pan_x, pan_y);
			event.preventDefault();
			return;
		}
		const zoom_sensitivity = event.ctrlKey || event.metaKey ? 0.00135 : 0.0023;
		const zoom_factor = Math.exp(delta.y * zoom_sensitivity);
		context.camera.zoom_view_at_screen(point.x, point.y, zoom_factor, zoom_factor);
		event.preventDefault();
	};
	return {
		shortcut_modifier_active,
		matches_canvas_double_activation,
		blur_active_editable_element,
		on_canvas_context_menu,
		on_canvas_pointer_down,
		on_canvas_pointer_move,
		on_canvas_pointer_up,
		on_canvas_pointer_leave,
		on_canvas_focus,
		on_canvas_blur,
		on_canvas_wheel
	};
}
