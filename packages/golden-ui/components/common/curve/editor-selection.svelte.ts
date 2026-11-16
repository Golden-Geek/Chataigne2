import type { NodeId } from '../../../types';
import type { SelectionMode, ActiveBoxSelection, ScreenRect, CanvasSelectionRect } from './types';
import type { CurveEditorBindings } from './editor-context';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorState } from './editor-state.svelte';
interface Context {
	readonly bindings: CurveEditorBindings;
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly state: ReturnType<typeof createCurveEditorState>;
}
export function createCurveEditorSelection(context: Context) {
	// Explicit input chooses exact nodes even when a field already projects to the same item.
	const publish_input_selection = (): void => {
		context.bindings.onselectioninput?.({
			key: context.bindings.selected_key_id,
			keys: [...context.bindings.selected_key_ids],
			segments: [...context.bindings.selected_curve_owner_key_ids]
		});
	};
	const same_node_id_array = (left: NodeId[], right: NodeId[]): boolean => {
		if (left.length !== right.length) {
			return false;
		}
		for (let index = 0; index < left.length; index += 1) {
			if (left[index] !== right[index]) {
				return false;
			}
		}
		return true;
	};
	const has_selected_key = (key_id: NodeId): boolean =>
		context.bindings.selected_key_ids.includes(key_id);
	const clear_selected_keys = (): void => {
		if (context.bindings.selected_key_id !== null) {
			context.bindings.selected_key_id = null;
		}
		if (context.bindings.selected_key_ids.length > 0) {
			context.bindings.selected_key_ids = [];
		}
	};
	const clear_selected_curve_owner_keys = (): void => {
		if (context.bindings.selected_curve_owner_key_ids.length > 0) {
			context.bindings.selected_curve_owner_key_ids = [];
		}
	};
	const set_single_selected_key = (key_id: NodeId | null): void => {
		if (key_id !== null) {
			clear_selected_curve_owner_keys();
		}
		context.bindings.selected_key_id = key_id;
		context.bindings.selected_key_ids = key_id === null ? [] : [key_id];
	};
	const add_selected_key = (key_id: NodeId): void => {
		clear_selected_curve_owner_keys();
		if (has_selected_key(key_id)) {
			if (context.bindings.selected_key_id === null) {
				context.bindings.selected_key_id = key_id;
			}
			return;
		}
		context.bindings.selected_key_ids = [...context.bindings.selected_key_ids, key_id];
		if (context.bindings.selected_key_id === null) {
			context.bindings.selected_key_id = key_id;
		}
	};
	const has_selected_curve_owner_key = (key_id: NodeId): boolean =>
		context.bindings.selected_curve_owner_key_ids.includes(key_id);
	const set_single_selected_curve_owner_key = (key_id: NodeId | null): void => {
		if (key_id !== null) {
			clear_selected_keys();
		}
		context.bindings.selected_curve_owner_key_ids = key_id === null ? [] : [key_id];
	};
	const add_selected_curve_owner_key = (key_id: NodeId): void => {
		clear_selected_keys();
		if (has_selected_curve_owner_key(key_id)) {
			return;
		}
		context.bindings.selected_curve_owner_key_ids = [
			...context.bindings.selected_curve_owner_key_ids,
			key_id
		];
	};
	const ordered_node_ids = (node_ids: Iterable<NodeId>): NodeId[] => {
		const selected = new Set(node_ids);
		const ordered: NodeId[] = [];
		for (const key of context.model.parsed_keys) {
			if (selected.has(key.node_id)) {
				ordered.push(key.node_id);
			}
		}
		return ordered;
	};
	const apply_selection_mode = (
		current_selection: NodeId[],
		hits: NodeId[],
		mode: SelectionMode
	): NodeId[] => {
		if (mode === 'replace') {
			return hits;
		}
		const next = new Set(current_selection);
		if (mode === 'add') {
			for (const key_id of hits) {
				next.add(key_id);
			}
			return ordered_node_ids(next);
		}
		for (const key_id of hits) {
			if (next.has(key_id)) {
				next.delete(key_id);
			} else {
				next.add(key_id);
			}
		}
		return ordered_node_ids(next);
	};
	const selected_frame_key_ids = (): NodeId[] => {
		const selected_ids = selected_key_ids_ordered();
		if (selected_ids.length > 0) {
			return selected_ids;
		}
		if (context.bindings.selected_curve_owner_key_ids.length === 0) {
			return [];
		}
		const owner_set = new Set(context.bindings.selected_curve_owner_key_ids);
		const framed = new Set<NodeId>();
		for (let index = 0; index < context.model.parsed_keys.length; index += 1) {
			const key = context.model.parsed_keys[index];
			if (!owner_set.has(key.node_id)) {
				continue;
			}
			framed.add(key.node_id);
			const next = context.model.parsed_keys[index + 1];
			if (next) {
				framed.add(next.node_id);
			}
		}
		return context.model.parsed_keys
			.filter((entry) => framed.has(entry.node_id))
			.map((entry) => entry.node_id);
	};
	const normalized_screen_rect = (x0: number, y0: number, x1: number, y1: number): ScreenRect => ({
		left: Math.min(x0, x1),
		right: Math.max(x0, x1),
		top: Math.min(y0, y1),
		bottom: Math.max(y0, y1)
	});
	const point_in_screen_rect = (x: number, y: number, rect: ScreenRect): boolean =>
		x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom;
	const box_selection_rect = (selection: ActiveBoxSelection): ScreenRect =>
		normalized_screen_rect(
			selection.start_screen_x,
			selection.start_screen_y,
			selection.current_screen_x,
			selection.current_screen_y
		);
	let active_box_selection_rect = $derived.by((): CanvasSelectionRect | null => {
		const selection = context.state.active_box_selection;
		if (!selection) {
			return null;
		}
		const rect = box_selection_rect(selection);
		return {
			left: rect.left,
			top: rect.top,
			width: rect.right - rect.left,
			height: rect.bottom - rect.top
		};
	});
	const collect_key_ids_in_screen_rect = (rect: ScreenRect): NodeId[] => {
		if (!context.state.interaction_transform) {
			return [];
		}
		const selected = new Set<NodeId>();
		for (const [key_id, point] of context.state.interaction_transform.key_screen_points.entries()) {
			if (point_in_screen_rect(point.x, point.y, rect)) {
				selected.add(key_id);
			}
		}
		return ordered_node_ids(selected);
	};
	const collect_curve_owner_key_ids_in_screen_rect = (rect: ScreenRect): NodeId[] => {
		if (!context.state.interaction_transform) {
			return [];
		}
		const selected = new Set<NodeId>();
		const curve_x = context.state.interaction_transform.curve_screen_x;
		const curve_y = context.state.interaction_transform.curve_screen_y;
		const owners = context.state.interaction_transform.curve_owner_key_ids;
		for (let index = 0; index + 1 < curve_x.length; index += 1) {
			const owner = owners[index];
			if (owner === null) {
				continue;
			}
			const midpoint_x = (curve_x[index] + curve_x[index + 1]) * 0.5;
			const midpoint_y = (curve_y[index] + curve_y[index + 1]) * 0.5;
			if (
				point_in_screen_rect(curve_x[index], curve_y[index], rect) ||
				point_in_screen_rect(curve_x[index + 1], curve_y[index + 1], rect) ||
				point_in_screen_rect(midpoint_x, midpoint_y, rect)
			) {
				selected.add(owner);
			}
		}
		return ordered_node_ids(selected);
	};
	const complete_box_selection = (active: ActiveBoxSelection): void => {
		const rect = box_selection_rect(active);
		const drag_size = Math.max(rect.right - rect.left, rect.bottom - rect.top);
		const drag_threshold = Math.max(3, context.state.rem_base_px * 0.24);
		const performed_drag = drag_size >= drag_threshold;
		if (!performed_drag) {
			if (active.mode === 'replace') {
				set_single_selected_key(null);
				set_single_selected_curve_owner_key(null);
				publish_input_selection();
			}
			return;
		}
		const key_hits = collect_key_ids_in_screen_rect(rect);
		const curve_hits = collect_curve_owner_key_ids_in_screen_rect(rect);
		const current_key_selection =
			context.bindings.selected_key_ids.length > 0
				? context.bindings.selected_key_ids
				: context.bindings.selected_key_id === null
					? []
					: [context.bindings.selected_key_id];
		if (key_hits.length > 0) {
			const next_key_selection = apply_selection_mode(current_key_selection, key_hits, active.mode);
			context.bindings.selected_key_ids = next_key_selection;
			context.bindings.selected_key_id = next_key_selection[0] ?? null;
			clear_selected_curve_owner_keys();
			publish_input_selection();
			return;
		}
		if (active.mode === 'replace' || curve_hits.length > 0) {
			clear_selected_keys();
		}
		context.bindings.selected_curve_owner_key_ids = apply_selection_mode(
			context.bindings.selected_curve_owner_key_ids,
			curve_hits,
			active.mode
		);
		publish_input_selection();
	};
	const clear_canvas_selection = (): void => {
		clear_selected_keys();
		clear_selected_curve_owner_keys();
		context.bindings.onselectionclear?.();
	};
	const selected_key_ids_ordered = (): NodeId[] => {
		if (context.bindings.selected_key_ids.length > 0) {
			const selected_set = new Set(context.bindings.selected_key_ids);
			return context.model.parsed_keys
				.filter((entry) => selected_set.has(entry.node_id))
				.map((entry) => entry.node_id);
		}
		return context.bindings.selected_key_id === null ? [] : [context.bindings.selected_key_id];
	};
	const set_all_keys_selected = (): void => {
		const all_ids = context.model.parsed_keys.map((entry) => entry.node_id);
		context.bindings.selected_key_ids = all_ids;
		context.bindings.selected_key_id = all_ids[0] ?? null;
		context.bindings.selected_curve_owner_key_ids = [];
		publish_input_selection();
	};
	return {
		publish_input_selection,
		same_node_id_array,
		has_selected_key,
		clear_selected_keys,
		clear_selected_curve_owner_keys,
		set_single_selected_key,
		add_selected_key,
		has_selected_curve_owner_key,
		set_single_selected_curve_owner_key,
		add_selected_curve_owner_key,
		ordered_node_ids,
		apply_selection_mode,
		selected_frame_key_ids,
		normalized_screen_rect,
		point_in_screen_rect,
		box_selection_rect,
		get active_box_selection_rect() {
			return active_box_selection_rect;
		},
		collect_key_ids_in_screen_rect,
		collect_curve_owner_key_ids_in_screen_rect,
		complete_box_selection,
		clear_canvas_selection,
		selected_key_ids_ordered,
		set_all_keys_selected
	};
}
