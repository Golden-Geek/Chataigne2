import {
	CURVE_NODE_TYPE,
	KEY_NODE_TYPE,
	KEY_ITEM_KIND,
	RANGE_NODE_TYPE,
	DECL_RANGE,
	DECL_RANGE_X,
	DECL_RANGE_Y,
	DECL_OUT_POSITION,
	DECL_OUT_VALUE,
	DECL_IN_POSITION,
	DECL_IN_VALUE,
	CURVE_EPSILON
} from './constants';
import {
	clamp,
	finite_or,
	param_number_value,
	make_curve_range_constraint,
	range_bounds_from_value,
	uniform_bounds_from_constraints
} from './values';
import { compile_curve } from './evaluation';
import { readCurveKeys } from './model';
import type { NodeId, UiNodeDto } from '../../../types';
import type {
	CurveRangeConstraint,
	CurveRangeState,
	ParsedCurveKey,
	BezierHandleAlignmentState,
	BezierHandleRef,
	BezierHandleSegment,
	BezierHandleControl,
	BezierHandleVisual
} from './types';
import type { CurveEditorDocument, CurveEditorBindings } from './editor-context';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorSculpt } from './editor-sculpt';
interface Context {
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly doc: CurveEditorDocument;
	readonly bindings: CurveEditorBindings;
	readonly sculpt: ReturnType<typeof createCurveEditorSculpt>;
}
export function createCurveEditorModel(context: Context) {
	let active_curve_draw_points = $derived.by(
		() =>
			context.state.active_curve_draw?.points.map((point) => ({
				position: point.position,
				value: point.value
			})) ?? []
	);
	let parsed_keys = $derived(
		readCurveKeys(
			context.doc.graphNodesById,
			context.doc.liveNode,
			context.state.drag_preview_by_key_id,
			context.state.easing_drag_preview_by_key_id
		)
	);
	let parsed_key_by_id = $derived.by(
		(): Map<NodeId, ParsedCurveKey> => new Map(parsed_keys.map((entry) => [entry.node_id, entry]))
	);
	const resolve_bezier_handle_segment = (
		start: ParsedCurveKey,
		end: ParsedCurveKey
	): BezierHandleSegment | null => {
		const easing = start.easing;
		if (!easing || easing.kind !== 'bezier') {
			return null;
		}
		const span = end.position - start.position;
		if (!(span > CURVE_EPSILON)) {
			return null;
		}
		const params = easing.params;
		const out_position = param_number_value(params[DECL_OUT_POSITION]?.value, 1 / 3);
		const out_value = param_number_value(params[DECL_OUT_VALUE]?.value, 1 / 3);
		const in_position = param_number_value(params[DECL_IN_POSITION]?.value, -1 / 3);
		const in_value = param_number_value(params[DECL_IN_VALUE]?.value, -1 / 3);
		let out_curve_position = finite_or(
			start.position + out_position * span,
			start.position + span / 3
		);
		let out_curve_value = finite_or(start.value + out_value, start.value + 1 / 3);
		let in_curve_position = finite_or(
			end.position + in_position * span,
			start.position + (span * 2) / 3
		);
		let in_curve_value = finite_or(end.value + in_value, end.value - 1 / 3);
		out_curve_position = clamp(out_curve_position, start.position, end.position);
		in_curve_position = clamp(in_curve_position, start.position, end.position);
		return {
			start_key_id: start.node_id,
			end_key_id: end.node_id,
			key_id: start.node_id,
			start_position: start.position,
			start_value: start.value,
			end_position: end.position,
			end_value: end.value,
			span,
			out_position,
			out_value,
			in_position,
			in_value,
			out_curve_position,
			out_curve_value,
			in_curve_position,
			in_curve_value,
			params
		};
	};
	let bezier_segment_by_owner_key_id = $derived.by((): Map<NodeId, BezierHandleSegment> => {
		const segments = new Map<NodeId, BezierHandleSegment>();
		for (let index = 0; index + 1 < parsed_keys.length; index += 1) {
			const start = parsed_keys[index];
			const end = parsed_keys[index + 1];
			const segment = resolve_bezier_handle_segment(start, end);
			if (!segment) {
				continue;
			}
			segments.set(segment.key_id, segment);
		}
		return segments;
	});
	let bezier_handle_owner_key_ids = $derived.by((): NodeId[] => {
		if (context.bindings.selected_key_ids.length > 0) {
			return context.bindings.selected_key_ids;
		}
		if (context.bindings.selected_key_id !== null) {
			return [context.bindings.selected_key_id];
		}
		if (context.bindings.selected_curve_owner_key_ids.length > 0) {
			return context.bindings.selected_curve_owner_key_ids;
		}
		return [];
	});
	let all_bezier_handle_segments = $derived.by((): BezierHandleSegment[] => [
		...bezier_segment_by_owner_key_id.values()
	]);
	let bezier_visible_anchor_key_ids = $derived.by((): NodeId[] => {
		if (bezier_handle_owner_key_ids.length === 0) {
			return [];
		}
		const owner_set = new Set(bezier_handle_owner_key_ids);
		const visible = new Set<NodeId>(bezier_handle_owner_key_ids);
		const show_neighbors_on_both_sides =
			context.bindings.selected_key_ids.length > 0 || context.bindings.selected_key_id !== null;
		for (let index = 0; index + 1 < parsed_keys.length; index += 1) {
			const start = parsed_keys[index];
			const end = parsed_keys[index + 1];
			if (!owner_set.has(start.node_id)) {
				if (!(show_neighbors_on_both_sides && owner_set.has(end.node_id))) {
					continue;
				}
				visible.add(start.node_id);
				continue;
			}
			// Selected curve/key means current and next anchors are involved.
			visible.add(end.node_id);
			if (show_neighbors_on_both_sides) {
				visible.add(start.node_id);
			}
		}
		return [...visible];
	});
	let bezier_handle_segments = $derived.by((): BezierHandleSegment[] => {
		const visible_anchor_set = new Set(bezier_visible_anchor_key_ids);
		if (visible_anchor_set.size === 0) {
			return [];
		}
		const segments: BezierHandleSegment[] = [];
		for (const segment of all_bezier_handle_segments) {
			if (
				!visible_anchor_set.has(segment.start_key_id) &&
				!visible_anchor_set.has(segment.end_key_id)
			) {
				continue;
			}
			segments.push(segment);
		}
		return segments;
	});
	const bezier_handle_control_id = (ref: BezierHandleRef): string => `${ref.key_id}:${ref.kind}`;
	let all_bezier_handle_controls = $derived.by((): BezierHandleControl[] => {
		const controls: BezierHandleControl[] = [];
		for (const segment of all_bezier_handle_segments) {
			controls.push({
				ref: { key_id: segment.key_id, kind: 'out' },
				anchor_key_id: segment.start_key_id,
				anchor_position: segment.start_position,
				anchor_value: segment.start_value,
				handle_position: segment.out_curve_position,
				handle_value: segment.out_curve_value,
				segment_start_position: segment.start_position,
				segment_end_position: segment.end_position,
				span: segment.span,
				params: segment.params
			});
			controls.push({
				ref: { key_id: segment.key_id, kind: 'in' },
				anchor_key_id: segment.end_key_id,
				anchor_position: segment.end_position,
				anchor_value: segment.end_value,
				handle_position: segment.in_curve_position,
				handle_value: segment.in_curve_value,
				segment_start_position: segment.start_position,
				segment_end_position: segment.end_position,
				span: segment.span,
				params: segment.params
			});
		}
		return controls;
	});
	let bezier_handle_controls = $derived.by((): BezierHandleControl[] => {
		const visible_anchor_set = new Set(bezier_visible_anchor_key_ids);
		if (visible_anchor_set.size === 0) {
			return [];
		}
		return all_bezier_handle_controls.filter((control) =>
			visible_anchor_set.has(control.anchor_key_id)
		);
	});
	let all_bezier_handle_control_by_id = $derived.by((): Map<string, BezierHandleControl> => {
		const controls = new Map<string, BezierHandleControl>();
		for (const control of all_bezier_handle_controls) {
			controls.set(bezier_handle_control_id(control.ref), control);
		}
		return controls;
	});
	let all_bezier_handle_refs_by_anchor_key_id = $derived.by((): Map<NodeId, BezierHandleRef[]> => {
		const mapping = new Map<NodeId, BezierHandleRef[]>();
		for (const control of all_bezier_handle_controls) {
			const existing = mapping.get(control.anchor_key_id) ?? [];
			existing.push(control.ref);
			mapping.set(control.anchor_key_id, existing);
		}
		return mapping;
	});
	let bezier_handle_alignment_by_anchor_key_id = $derived.by(
		(): Map<NodeId, BezierHandleAlignmentState> => {
			const mapping = new Map<NodeId, BezierHandleAlignmentState>();
			for (const [anchor_key_id, refs] of all_bezier_handle_refs_by_anchor_key_id.entries()) {
				if (refs.length < 2) {
					mapping.set(anchor_key_id, 'single');
					continue;
				}
				const first_control = all_bezier_handle_control_by_id.get(
					bezier_handle_control_id(refs[0])
				);
				const second_control = all_bezier_handle_control_by_id.get(
					bezier_handle_control_id(refs[1])
				);
				if (!first_control || !second_control) {
					mapping.set(anchor_key_id, 'single');
					continue;
				}
				const aligned = context.sculpt.vectors_are_aligned_and_opposite(
					first_control.handle_position - first_control.anchor_position,
					first_control.handle_value - first_control.anchor_value,
					second_control.handle_position - second_control.anchor_position,
					second_control.handle_value - second_control.anchor_value
				);
				mapping.set(anchor_key_id, aligned ? 'aligned' : 'unlocked');
			}
			return mapping;
		}
	);
	let bezier_handle_visuals = $derived.by((): BezierHandleVisual[] =>
		bezier_handle_controls.map((control) => ({
			anchor_key_id: control.anchor_key_id,
			key_id: control.ref.key_id,
			kind: control.ref.kind,
			anchor_position: control.anchor_position,
			anchor_value: control.anchor_value,
			handle_position: control.handle_position,
			handle_value: control.handle_value,
			alignment_state:
				bezier_handle_alignment_by_anchor_key_id.get(control.anchor_key_id) ?? 'single'
		}))
	);
	let editable_curve_range_node = $derived.by((): UiNodeDto | null => {
		const nodes_by_id = context.doc.graphNodesById;
		if (!nodes_by_id || context.doc.liveNode.node_type !== CURVE_NODE_TYPE) {
			return null;
		}
		for (const child_id of context.doc.liveNode.children) {
			const child = nodes_by_id.get(child_id);
			if (!child || child.decl_id !== DECL_RANGE || child.node_type !== RANGE_NODE_TYPE) {
				continue;
			}
			return child;
		}
		return null;
	});
	let curve_range_state = $derived.by((): CurveRangeState | null => {
		const range_node = editable_curve_range_node;
		if (range_node) {
			const nodes_by_id = context.doc.graphNodesById;
			if (!nodes_by_id) {
				return null;
			}
			let x_bounds: {
				min: number;
				max: number;
			} | null = null;
			let y_bounds: {
				min: number;
				max: number;
			} | null = null;
			for (const child_id of range_node.children) {
				const child = nodes_by_id.get(child_id);
				if (!child || child.data.kind !== 'parameter') {
					continue;
				}
				if (child.decl_id === DECL_RANGE_X) {
					x_bounds = range_bounds_from_value(child.data.param.value);
					continue;
				}
				if (child.decl_id === DECL_RANGE_Y) {
					y_bounds = range_bounds_from_value(child.data.param.value);
				}
			}
			if (!x_bounds || !y_bounds) {
				return null;
			}
			const bounds = make_curve_range_constraint(
				x_bounds.min,
				x_bounds.max,
				y_bounds.min,
				y_bounds.max
			);
			if (!bounds) {
				return null;
			}
			return {
				bounds,
				active: range_node.meta.enabled,
				range_node
			};
		}
		let x_bounds: {
			min: number;
			max: number;
		} | null = null;
		let y_bounds: {
			min: number;
			max: number;
		} | null = null;
		for (const key of parsed_keys) {
			if (!x_bounds && key.position_param) {
				x_bounds = uniform_bounds_from_constraints(key.position_param.constraints);
			}
			if (!y_bounds && key.value_param) {
				y_bounds = uniform_bounds_from_constraints(key.value_param.constraints);
			}
			if (x_bounds && y_bounds) {
				break;
			}
		}
		if (!x_bounds || !y_bounds) {
			return null;
		}
		const bounds = make_curve_range_constraint(
			x_bounds.min,
			x_bounds.max,
			y_bounds.min,
			y_bounds.max
		);
		if (!bounds) {
			return null;
		}
		return {
			bounds,
			active: true,
			range_node: null
		};
	});
	let active_curve_range_constraint = $derived.by((): CurveRangeConstraint | null => {
		if (!curve_range_state?.active) {
			return null;
		}
		return curve_range_state.bounds;
	});
	let selected_key = $derived.by((): ParsedCurveKey | null => {
		if (context.bindings.selected_key_id === null) {
			return null;
		}
		return parsed_key_by_id.get(context.bindings.selected_key_id) ?? null;
	});
	let key_creatable_item = $derived.by(
		() =>
			context.doc.liveNode.creatable_user_items.find(
				(item) => item.node_type === KEY_NODE_TYPE || item.item_kind === KEY_ITEM_KIND
			) ?? null
	);
	let compiled_curve = $derived.by(() => compile_curve(parsed_keys));
	return {
		get active_curve_draw_points() {
			return active_curve_draw_points;
		},
		get parsed_keys() {
			return parsed_keys;
		},
		get parsed_key_by_id() {
			return parsed_key_by_id;
		},
		resolve_bezier_handle_segment,
		get bezier_segment_by_owner_key_id() {
			return bezier_segment_by_owner_key_id;
		},
		get bezier_handle_owner_key_ids() {
			return bezier_handle_owner_key_ids;
		},
		get all_bezier_handle_segments() {
			return all_bezier_handle_segments;
		},
		get bezier_visible_anchor_key_ids() {
			return bezier_visible_anchor_key_ids;
		},
		get bezier_handle_segments() {
			return bezier_handle_segments;
		},
		bezier_handle_control_id,
		get all_bezier_handle_controls() {
			return all_bezier_handle_controls;
		},
		get bezier_handle_controls() {
			return bezier_handle_controls;
		},
		get all_bezier_handle_control_by_id() {
			return all_bezier_handle_control_by_id;
		},
		get all_bezier_handle_refs_by_anchor_key_id() {
			return all_bezier_handle_refs_by_anchor_key_id;
		},
		get bezier_handle_alignment_by_anchor_key_id() {
			return bezier_handle_alignment_by_anchor_key_id;
		},
		get bezier_handle_visuals() {
			return bezier_handle_visuals;
		},
		get editable_curve_range_node() {
			return editable_curve_range_node;
		},
		get curve_range_state() {
			return curve_range_state;
		},
		get active_curve_range_constraint() {
			return active_curve_range_constraint;
		},
		get selected_key() {
			return selected_key;
		},
		get key_creatable_item() {
			return key_creatable_item;
		},
		get compiled_curve() {
			return compiled_curve;
		}
	};
}
