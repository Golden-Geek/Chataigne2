import type { NodeId, ParamValue, UiNodeDto, UiParamConstraints } from '../../../types';

export interface CurveSelection {
	key: NodeId | null;
	keys: NodeId[];
	segments: NodeId[];
}

export type CurveEasingKind =
	| 'linear'
	| 'bezier'
	| 'hold'
	| 'steps'
	| 'shape'
	| 'perlinNoise'
	| 'elastic'
	| 'bounce'
	| 'random'
	| 'script';
export type CurveStepMode = 'stepSize' | 'numSteps';
export type CurvePhaseMode = 'frequency' | 'numPhases';
export type CurveShape = 'sine' | 'triangle' | 'saw' | 'reverseSaw' | 'square';

export interface ParamNodeRef {
	node_id: NodeId;
	uuid: string;
	value: ParamValue;
	constraints: UiParamConstraints;
}

export interface CurveRangeConstraint {
	x_min: number;
	x_max: number;
	y_min: number;
	y_max: number;
}

export interface CurveRangeState {
	bounds: CurveRangeConstraint;
	active: boolean;
	range_node: UiNodeDto | null;
}

export interface ParsedEasing {
	node_id: NodeId;
	kind: CurveEasingKind;
	params: Record<string, ParamNodeRef>;
}

export interface ParsedCurveKey {
	node_id: NodeId;
	position: number;
	value: number;
	source_index: number;
	position_param?: ParamNodeRef;
	value_param?: ParamNodeRef;
	easing?: ParsedEasing;
}

export interface EasingKindOption {
	value: CurveEasingKind;
	label: string;
}

export interface CubicPolynomial {
	a: number;
	b: number;
	c: number;
	d: number;
}

export type CompiledSegmentEasing =
	| { kind: 'linear' }
	| { kind: 'elastic' }
	| { kind: 'bounce' }
	| { kind: 'hold' }
	| {
			kind: 'bezier';
			x: CubicPolynomial;
			y: CubicPolynomial;
			start_position: number;
			inv_span: number;
	  }
	| { kind: 'steps'; steps: number }
	| {
			kind: 'shape';
			shape: CurveShape;
			amplitude: number;
			phase_cycles: number;
			fade_in: number;
			fade_out: number;
	  }
	| {
			kind: 'perlinNoise';
			frequency: number;
			amplitude: number;
			octaves: number;
			fade_in: number;
			fade_out: number;
			phase: number;
			seed: number;
	  }
	| {
			kind: 'random';
			frequency: number;
			fade_in: number;
			fade_out: number;
			seed: number;
	  }
	| { kind: 'script'; source: string };

export interface CompiledCurveSegment {
	start_position: number;
	end_position: number;
	start_value: number;
	end_value: number;
	value_delta: number;
	inv_span: number;
	easing: CompiledSegmentEasing;
}

export interface CompiledCurveKey {
	node_id: NodeId;
	position: number;
	value: number;
	easing?: ParsedEasing;
}

export interface CompiledCurve {
	keys: CompiledCurveKey[];
	segments: CompiledCurveSegment[];
}

export interface CanvasTransform {
	plot_left: number;
	plot_top: number;
	plot_width: number;
	plot_height: number;
	x_min: number;
	x_max: number;
	y_min: number;
	y_max: number;
	key_screen_points: Map<NodeId, { x: number; y: number }>;
	curve_screen_x: Float64Array;
	curve_screen_y: Float64Array;
	curve_owner_key_ids: Array<NodeId | null>;
}

export interface DragPreview {
	position: number;
	value: number;
}

export type BezierHandleKind = 'out' | 'in';
export type BezierHandleAlignmentState = 'single' | 'aligned' | 'unlocked';

export interface BezierHandleRef {
	key_id: NodeId;
	kind: BezierHandleKind;
}

export interface BezierHandleSegment {
	start_key_id: NodeId;
	end_key_id: NodeId;
	key_id: NodeId;
	start_position: number;
	start_value: number;
	end_position: number;
	end_value: number;
	span: number;
	out_position: number;
	out_value: number;
	in_position: number;
	in_value: number;
	out_curve_position: number;
	out_curve_value: number;
	in_curve_position: number;
	in_curve_value: number;
	params: Record<string, ParamNodeRef>;
}

export interface BezierHandleControl {
	ref: BezierHandleRef;
	anchor_key_id: NodeId;
	anchor_position: number;
	anchor_value: number;
	handle_position: number;
	handle_value: number;
	segment_start_position: number;
	segment_end_position: number;
	span: number;
	params: Record<string, ParamNodeRef>;
}

export interface BezierHandleVisual {
	anchor_key_id: NodeId;
	key_id: NodeId;
	kind: BezierHandleKind;
	anchor_position: number;
	anchor_value: number;
	handle_position: number;
	handle_value: number;
	alignment_state: BezierHandleAlignmentState;
}

export interface BezierEasingPreview {
	out_position: number;
	out_value: number;
	in_position: number;
	in_value: number;
}

export interface CurveCreatePlan {
	position: number;
	value: number;
	split_easing: PendingSplitEasing | null;
	easing_values: Record<string, ParamValue> | null;
	existing_easing_updates: Array<{
		key_id: NodeId;
		values: Record<string, ParamValue>;
	}>;
}

export interface PendingSplitEasing {
	source_start_key_id: NodeId;
	source_end_key_id: NodeId;
	left_param_values: Record<string, ParamValue>;
	right_param_values: Record<string, ParamValue>;
	preserved_aligned_handle_updates?: Array<{
		key_id: NodeId;
		values: Record<string, ParamValue>;
	}>;
}

export interface AddKeyOptions {
	preserve_curve_shape?: boolean;
	preferred_owner_key_id?: NodeId | null;
	easing_values?: Record<string, ParamValue> | null;
	existing_easing_updates?: Array<{
		key_id: NodeId;
		values: Record<string, ParamValue>;
	}>;
}

export interface ActiveKeyDrag {
	pointer_id: number;
	anchor_key_id: NodeId;
	anchor_start_position: number;
	anchor_start_value: number;
	origins: Array<{ key_id: NodeId; position: number; value: number }>;
	position_delta_constraint: KeyDragPositionConstraint | null;
	bezier_alignment_constraints: KeyDragBezierAlignmentConstraint[];
	touched_bezier_key_ids: Set<NodeId>;
}

export interface KeyDragPositionConstraint {
	min_delta_position: number;
	max_delta_position: number;
}

export interface KeyDragBezierAlignmentConstraint {
	anchor_key_id: NodeId;
	previous_key_id: NodeId;
	next_key_id: NodeId;
	previous_owner_key_id: NodeId;
	anchor_owner_key_id: NodeId;
	left_span: number;
	right_span: number;
	previous_out_vector_x: number;
	previous_out_vector_y: number;
	in_vector_x: number;
	in_vector_y: number;
	out_vector_x: number;
	out_vector_y: number;
	next_in_vector_x: number;
	next_in_vector_y: number;
}

export interface ActiveBezierHandleDrag {
	pointer_id: number;
	ref: BezierHandleRef;
	opposite_ref: BezierHandleRef | null;
	opposite_length: number;
	touched_key_ids: Set<NodeId>;
}

export interface CurveSegmentHit {
	owner_key_id: NodeId;
	curve_position: number;
	curve_value: number;
}

export interface CurveOppositeAlignment {
	anchor_key_id: NodeId;
	opposite_ref: BezierHandleRef;
	opposite_length: number;
}

export interface ActiveCurveDrag {
	pointer_id: number;
	owner_key_id: NodeId;
	start_screen_x: number;
	start_curve_position: number;
	start_curve_value: number;
	segment: BezierHandleSegment;
	normalized_t: number;
	base_out_curve_position: number;
	base_out_curve_value: number;
	base_in_curve_position: number;
	base_in_curve_value: number;
	out_length: number;
	in_length: number;
	start_opposite_alignment: CurveOppositeAlignment | null;
	end_opposite_alignment: CurveOppositeAlignment | null;
	touched_key_ids: Set<NodeId>;
}

export interface ActiveCanvasPan {
	pointer_id: number;
	start_screen_x: number;
	start_screen_y: number;
	start_bounds: CurveViewBounds;
}

export interface RecordedCurveDrawPoint {
	bucket: number;
	position: number;
	value: number;
}

export interface ActiveCurveDraw {
	pointer_id: number;
	points: RecordedCurveDrawPoint[];
	path_buckets: number[];
}

export type CurveContextMenuTarget =
	{ kind: 'key'; key_id: NodeId } | { kind: 'curve'; key_id: NodeId } | { kind: 'all' };

export interface CanvasPrimaryDownSample {
	time_stamp: number;
	screen_x: number;
	screen_y: number;
}

export type SelectionMode = 'replace' | 'add' | 'toggle';

export interface ActiveBoxSelection {
	pointer_id: number;
	start_screen_x: number;
	start_screen_y: number;
	current_screen_x: number;
	current_screen_y: number;
	mode: SelectionMode;
}

export interface ScreenRect {
	left: number;
	right: number;
	top: number;
	bottom: number;
}

export interface CanvasSelectionRect {
	left: number;
	top: number;
	width: number;
	height: number;
}

export interface CurveViewBounds {
	x_min: number;
	x_max: number;
	y_min: number;
	y_max: number;
}
