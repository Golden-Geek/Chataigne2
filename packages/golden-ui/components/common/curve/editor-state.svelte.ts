import type { NodeId } from '../../../types';
import type { ContextMenuAnchor } from '../context-menu';
import type {
	CanvasTransform,
	DragPreview,
	BezierHandleRef,
	BezierEasingPreview,
	ActiveKeyDrag,
	ActiveBezierHandleDrag,
	ActiveCurveDrag,
	ActiveCanvasPan,
	ActiveCurveDraw,
	CurveContextMenuTarget,
	CanvasPrimaryDownSample,
	ActiveBoxSelection,
	CurveViewBounds
} from './types';

export function createCurveEditorState() {
	let rem_base_px = $state(16);
	let hover_key_id = $state<NodeId | null>(null);
	let hover_curve_owner_key_id = $state<NodeId | null>(null);
	let hover_bezier_handle = $state<BezierHandleRef | null>(null);
	let drag_preview_by_key_id = $state<Map<NodeId, DragPreview>>(new Map());
	let easing_drag_preview_by_key_id = $state<Map<NodeId, BezierEasingPreview>>(new Map());
	let fixed_view_bounds = $state<CurveViewBounds | null>(null);
	let input_view_owner: string | null | undefined;
	let canvas_element = $state<HTMLCanvasElement | null>(null);
	let active_drag = $state<ActiveKeyDrag | null>(null);
	let active_bezier_handle_drag = $state<ActiveBezierHandleDrag | null>(null);
	let active_curve_drag = $state<ActiveCurveDrag | null>(null);
	let active_box_selection = $state<ActiveBoxSelection | null>(null);
	let active_canvas_pan = $state<ActiveCanvasPan | null>(null);
	let active_curve_draw = $state<ActiveCurveDraw | null>(null);
	let hover_curve_position = $state<{
		position: number;
		value: number;
	} | null>(null);
	let is_canvas_focused = $state(false);
	let curve_context_menu_open = $state(false);
	let curve_context_menu_anchor = $state<ContextMenuAnchor | null>(null);
	let curve_context_menu_target = $state<CurveContextMenuTarget | null>(null);
	let last_canvas_primary_down = $state<CanvasPrimaryDownSample | null>(null);
	let interaction_transform = $state<CanvasTransform | null>(null);
	let view_bounds_animation_frame = 0;
	let view_bounds_animation_target: CurveViewBounds | null = null;
	return {
		get rem_base_px() {
			return rem_base_px;
		},
		set rem_base_px(value: typeof rem_base_px) {
			rem_base_px = value;
		},
		get hover_key_id() {
			return hover_key_id;
		},
		set hover_key_id(value: typeof hover_key_id) {
			hover_key_id = value;
		},
		get hover_curve_owner_key_id() {
			return hover_curve_owner_key_id;
		},
		set hover_curve_owner_key_id(value: typeof hover_curve_owner_key_id) {
			hover_curve_owner_key_id = value;
		},
		get hover_bezier_handle() {
			return hover_bezier_handle;
		},
		set hover_bezier_handle(value: typeof hover_bezier_handle) {
			hover_bezier_handle = value;
		},
		get drag_preview_by_key_id() {
			return drag_preview_by_key_id;
		},
		set drag_preview_by_key_id(value: typeof drag_preview_by_key_id) {
			drag_preview_by_key_id = value;
		},
		get easing_drag_preview_by_key_id() {
			return easing_drag_preview_by_key_id;
		},
		set easing_drag_preview_by_key_id(value: typeof easing_drag_preview_by_key_id) {
			easing_drag_preview_by_key_id = value;
		},
		get fixed_view_bounds() {
			return fixed_view_bounds;
		},
		set fixed_view_bounds(value: typeof fixed_view_bounds) {
			fixed_view_bounds = value;
		},
		get input_view_owner() {
			return input_view_owner;
		},
		set input_view_owner(value: typeof input_view_owner) {
			input_view_owner = value;
		},
		get canvas_element() {
			return canvas_element;
		},
		set canvas_element(value: typeof canvas_element) {
			canvas_element = value;
		},
		get active_drag() {
			return active_drag;
		},
		set active_drag(value: typeof active_drag) {
			active_drag = value;
		},
		get active_bezier_handle_drag() {
			return active_bezier_handle_drag;
		},
		set active_bezier_handle_drag(value: typeof active_bezier_handle_drag) {
			active_bezier_handle_drag = value;
		},
		get active_curve_drag() {
			return active_curve_drag;
		},
		set active_curve_drag(value: typeof active_curve_drag) {
			active_curve_drag = value;
		},
		get active_box_selection() {
			return active_box_selection;
		},
		set active_box_selection(value: typeof active_box_selection) {
			active_box_selection = value;
		},
		get active_canvas_pan() {
			return active_canvas_pan;
		},
		set active_canvas_pan(value: typeof active_canvas_pan) {
			active_canvas_pan = value;
		},
		get active_curve_draw() {
			return active_curve_draw;
		},
		set active_curve_draw(value: typeof active_curve_draw) {
			active_curve_draw = value;
		},
		get hover_curve_position() {
			return hover_curve_position;
		},
		set hover_curve_position(value: typeof hover_curve_position) {
			hover_curve_position = value;
		},
		get is_canvas_focused() {
			return is_canvas_focused;
		},
		set is_canvas_focused(value: typeof is_canvas_focused) {
			is_canvas_focused = value;
		},
		get curve_context_menu_open() {
			return curve_context_menu_open;
		},
		set curve_context_menu_open(value: typeof curve_context_menu_open) {
			curve_context_menu_open = value;
		},
		get curve_context_menu_anchor() {
			return curve_context_menu_anchor;
		},
		set curve_context_menu_anchor(value: typeof curve_context_menu_anchor) {
			curve_context_menu_anchor = value;
		},
		get curve_context_menu_target() {
			return curve_context_menu_target;
		},
		set curve_context_menu_target(value: typeof curve_context_menu_target) {
			curve_context_menu_target = value;
		},
		get last_canvas_primary_down() {
			return last_canvas_primary_down;
		},
		set last_canvas_primary_down(value: typeof last_canvas_primary_down) {
			last_canvas_primary_down = value;
		},
		get interaction_transform() {
			return interaction_transform;
		},
		set interaction_transform(value: typeof interaction_transform) {
			interaction_transform = value;
		},
		get view_bounds_animation_frame() {
			return view_bounds_animation_frame;
		},
		set view_bounds_animation_frame(value: typeof view_bounds_animation_frame) {
			view_bounds_animation_frame = value;
		},
		get view_bounds_animation_target() {
			return view_bounds_animation_target;
		},
		set view_bounds_animation_target(value: typeof view_bounds_animation_target) {
			view_bounds_animation_target = value;
		}
	};
}
