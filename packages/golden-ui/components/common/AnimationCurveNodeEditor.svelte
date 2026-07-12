<script lang="ts">
	import { appState } from '../../store/workbench.svelte';
	import {
		createUiEditSession,
		sendCreateUserItemIntent,
		sendFitAnimationCurvePathIntent,
		sendRemoveNodesIntent,
		sendSetParamIntent,
		type UiEditSession
	} from '../../store/ui-intents';
	import { registerCommandHandler } from '../../store/commands.svelte';
	import { curveClipboardState, type CurveClipboardKey } from '../../store/curve-clipboard.svelte';
	import type { NodeId, ParamValue, UiNodeDto, UiParamConstraints } from '../../types';
	import AnimationCurveCanvas from './AnimationCurveCanvas.svelte';
	import ContextMenu from './ContextMenu.svelte';
	import type { ContextMenuAnchor, ContextMenuItem } from './context-menu';

	type CurveEasingKind =
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
	type CurveStepMode = 'stepSize' | 'numSteps';
	type CurvePhaseMode = 'frequency' | 'numPhases';
	type CurveShape = 'sine' | 'triangle' | 'saw' | 'reverseSaw' | 'square';

	interface ParamNodeRef {
		node_id: NodeId;
		value: ParamValue;
		enabled: boolean;
		constraints: UiParamConstraints;
	}

	interface CurveRangeConstraint {
		x_min: number;
		x_max: number;
		y_min: number;
		y_max: number;
	}

	interface CurveRangeState {
		bounds: CurveRangeConstraint;
		active: boolean;
		range_node: UiNodeDto | null;
	}

	interface ParsedEasing {
		node_id: NodeId;
		kind: CurveEasingKind;
		params: Record<string, ParamNodeRef>;
	}

	interface ParsedCurveKey {
		node_id: NodeId;
		position: number;
		value: number;
		source_index: number;
		position_param?: ParamNodeRef;
		value_param?: ParamNodeRef;
		easing?: ParsedEasing;
	}

	interface EasingKindOption {
		value: CurveEasingKind;
		label: string;
	}

	interface CubicPolynomial {
		a: number;
		b: number;
		c: number;
		d: number;
	}

	type CompiledSegmentEasing =
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

	interface CompiledCurveSegment {
		start_position: number;
		end_position: number;
		start_value: number;
		end_value: number;
		value_delta: number;
		inv_span: number;
		easing: CompiledSegmentEasing;
	}

	interface CompiledCurveKey {
		node_id: NodeId;
		position: number;
		value: number;
		easing?: ParsedEasing;
	}

	interface CompiledCurve {
		keys: CompiledCurveKey[];
		segments: CompiledCurveSegment[];
	}

	interface CanvasTransform {
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

	interface DragPreview {
		position: number;
		value: number;
	}

	type BezierHandleKind = 'out' | 'in';
	type BezierHandleAlignmentState = 'single' | 'aligned' | 'unlocked';

	interface BezierHandleRef {
		key_id: NodeId;
		kind: BezierHandleKind;
	}

	interface BezierHandleSegment {
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

	interface BezierHandleControl {
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

	interface BezierHandleVisual {
		anchor_key_id: NodeId;
		key_id: NodeId;
		kind: BezierHandleKind;
		anchor_position: number;
		anchor_value: number;
		handle_position: number;
		handle_value: number;
		alignment_state: BezierHandleAlignmentState;
	}

	interface BezierEasingPreview {
		out_position: number;
		out_value: number;
		in_position: number;
		in_value: number;
	}

	interface PendingCreateTarget {
		known_key_ids: Set<NodeId>;
		created_key_id: NodeId | null;
		position: number;
		value: number;
		split_easing: PendingSplitEasing | null;
		easing_values: Record<string, ParamValue> | null;
		existing_easing_updates: Array<{
			key_id: NodeId;
			values: Record<string, ParamValue>;
		}>;
		edit_session: UiEditSession | null;
	}

	interface PendingSplitEasing {
		source_start_key_id: NodeId;
		source_end_key_id: NodeId;
		left_param_values: Record<string, ParamValue>;
		right_param_values: Record<string, ParamValue>;
		preserved_aligned_handle_updates?: Array<{
			key_id: NodeId;
			values: Record<string, ParamValue>;
		}>;
	}

	interface AddKeyOptions {
		preserve_curve_shape?: boolean;
		preferred_owner_key_id?: NodeId | null;
		easing_values?: Record<string, ParamValue> | null;
		existing_easing_updates?: Array<{
			key_id: NodeId;
			values: Record<string, ParamValue>;
		}>;
	}

	interface ActiveKeyDrag {
		pointer_id: number;
		anchor_key_id: NodeId;
		anchor_start_position: number;
		anchor_start_value: number;
		origins: Array<{ key_id: NodeId; position: number; value: number }>;
		position_delta_constraint: KeyDragPositionConstraint | null;
		bezier_alignment_constraints: KeyDragBezierAlignmentConstraint[];
		touched_bezier_key_ids: Set<NodeId>;
	}

	interface KeyDragPositionConstraint {
		min_delta_position: number;
		max_delta_position: number;
	}

	interface KeyDragBezierAlignmentConstraint {
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

	interface ActiveBezierHandleDrag {
		pointer_id: number;
		ref: BezierHandleRef;
		opposite_ref: BezierHandleRef | null;
		opposite_length: number;
		touched_key_ids: Set<NodeId>;
	}

	interface CurveSegmentHit {
		owner_key_id: NodeId;
		curve_position: number;
		curve_value: number;
	}

	interface CurveOppositeAlignment {
		anchor_key_id: NodeId;
		opposite_ref: BezierHandleRef;
		opposite_length: number;
	}

	interface ActiveCurveDrag {
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

	interface ActiveCanvasPan {
		pointer_id: number;
		start_screen_x: number;
		start_screen_y: number;
		start_bounds: CurveViewBounds;
	}

	interface RecordedCurveDrawPoint {
		bucket: number;
		position: number;
		value: number;
	}

	interface ActiveCurveDraw {
		pointer_id: number;
		points: RecordedCurveDrawPoint[];
		path_buckets: number[];
	}

	type CurveContextMenuTarget =
		| { kind: 'key'; key_id: NodeId }
		| { kind: 'curve'; key_id: NodeId }
		| { kind: 'all' };

	interface CanvasPrimaryDownSample {
		time_stamp: number;
		screen_x: number;
		screen_y: number;
	}

	type SelectionMode = 'replace' | 'add' | 'toggle';

	interface ActiveBoxSelection {
		pointer_id: number;
		start_screen_x: number;
		start_screen_y: number;
		current_screen_x: number;
		current_screen_y: number;
		mode: SelectionMode;
	}

	interface ScreenRect {
		left: number;
		right: number;
		top: number;
		bottom: number;
	}

	interface CanvasSelectionRect {
		left: number;
		top: number;
		width: number;
		height: number;
	}

	interface CurveViewBounds {
		x_min: number;
		x_max: number;
		y_min: number;
		y_max: number;
	}

	let {
		curveNode,
		selected_key_id = $bindable<NodeId | null>(null),
		selected_key_ids = $bindable<NodeId[]>([]),
		selected_curve_owner_key_ids = $bindable<NodeId[]>([]),
		showGrid = true,
		showNumbers = true,
		showBounds = true,
		constrainKeysToNeighbors = true,
		canvasHeight = 'min(26rem, 38vh)'
	}: {
		curveNode: UiNodeDto;
		selected_key_id?: NodeId | null;
		selected_key_ids?: NodeId[];
		selected_curve_owner_key_ids?: NodeId[];
		showGrid?: boolean;
		showNumbers?: boolean;
		showBounds?: boolean;
		constrainKeysToNeighbors?: boolean;
		canvasHeight?: string;
	} = $props();

	const CURVE_NODE_TYPE = 'animation_curve';
	const KEY_NODE_TYPE = 'animation_curve_key';
	const KEY_ITEM_KIND = 'animation_curve_key';
	const EASING_NODE_TYPE = 'animation_curve_easing';
	const RANGE_NODE_TYPE = 'animation_curve_range';

	const DECL_POSITION = 'position';
	const DECL_VALUE = 'value';
	const DECL_EASING = 'easing';
	const DECL_RANGE = 'range';
	const DECL_RANGE_X = 'x';
	const DECL_RANGE_Y = 'y';
	const DECL_KIND = 'kind';
	const DECL_OUT_POSITION = 'out_position';
	const DECL_OUT_VALUE = 'out_value';
	const DECL_IN_POSITION = 'in_position';
	const DECL_IN_VALUE = 'in_value';
	const DECL_STEP_MODE = 'step_mode';
	const DECL_STEP_SIZE = 'step_size';
	const DECL_NUM_STEPS = 'num_steps';
	const DECL_SHAPE = 'shape';
	const DECL_AMPLITUDE = 'amplitude';
	const DECL_PHASE_MODE = 'phase_mode';
	const DECL_FREQUENCY = 'frequency';
	const DECL_NUM_PHASES = 'num_phases';
	const DECL_FADE_IN = 'fade_in';
	const DECL_FADE_OUT = 'fade_out';
	const DECL_OCTAVES = 'octaves';
	const DECL_PHASE = 'phase';
	const DECL_SEED = 'seed';
	const DECL_SCRIPT_SOURCE = 'script_source';

	const CURVE_EPSILON = 1e-12;
	const MAX_PERLIN_OCTAVES = 12;
	const MIN_VIEW_SPAN = 1e-6;
	const MAX_VIEW_SPAN = 1e9;
	const CURVE_SCULPT_SHIFT_FULL_STRENGTH_PX = 8;
	const CURVE_DRAW_FIT_MAX_ERROR_PX = 4.5;
	const CURVE_DRAW_FIT_MAX_KEYS = 12;
	const VIEW_BOUNDS_ANIMATION_DURATION_MS = 220;
	const KEY_HIT_RADIUS_REM = 0.7;
	const CURVE_HIT_RADIUS_REM = 0.7;
	const BEZIER_HANDLE_HIT_RADIUS_REM = 0.64;
	const CANVAS_DOUBLE_CLICK_WINDOW_MS = 320;
	const CANVAS_DOUBLE_CLICK_RADIUS_PX = 7;
	const DEFAULT_EASING_KIND_ORDER: CurveEasingKind[] = [
		'linear',
		'bezier',
		'hold',
		'steps',
		'shape',
		'perlinNoise',
		'elastic',
		'bounce',
		'random',
		'script'
	];
	const EASING_KIND_LABELS: Record<CurveEasingKind, string> = {
		linear: 'Linear',
		bezier: 'Bezier',
		hold: 'Hold',
		steps: 'Steps',
		shape: 'Shape',
		perlinNoise: 'Perlin Noise',
		elastic: 'Elastic',
		bounce: 'Bounce',
		random: 'Random',
		script: 'Script'
	};

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let liveNode: UiNodeDto = $derived(
		session?.graph.state.nodesById.get(curveNode.node_id) ?? curveNode
	);

	let rem_base_px = $state(16);
	let hover_key_id = $state<NodeId | null>(null);
	let hover_curve_owner_key_id = $state<NodeId | null>(null);
	let hover_bezier_handle = $state<BezierHandleRef | null>(null);
	let drag_preview_by_key_id = $state<Map<NodeId, DragPreview>>(new Map());
	let easing_drag_preview_by_key_id = $state<Map<NodeId, BezierEasingPreview>>(new Map());
	let pending_create_target = $state<PendingCreateTarget | null>(null);
	let adding_key = $state(false);
	let fixed_view_bounds = $state<CurveViewBounds | null>(null);
	let canvas_element = $state<HTMLCanvasElement | null>(null);
	let active_drag = $state<ActiveKeyDrag | null>(null);
	let active_bezier_handle_drag = $state<ActiveBezierHandleDrag | null>(null);
	let active_curve_drag = $state<ActiveCurveDrag | null>(null);
	let active_box_selection = $state<ActiveBoxSelection | null>(null);
	let active_canvas_pan = $state<ActiveCanvasPan | null>(null);
	let active_curve_draw = $state<ActiveCurveDraw | null>(null);
	let drag_edit_session = $state<UiEditSession | null>(null);
	let drag_edit_session_begin_promise: Promise<void> | null = null;
	let hover_curve_position = $state<{ position: number; value: number } | null>(null);
	let is_canvas_focused = $state(false);
	let curve_context_menu_open = $state(false);
	let curve_context_menu_anchor = $state<ContextMenuAnchor | null>(null);
	let curve_context_menu_target = $state<CurveContextMenuTarget | null>(null);
	let last_canvas_primary_down = $state<CanvasPrimaryDownSample | null>(null);

	let interaction_transform = $state<CanvasTransform | null>(null);
	let queued_drag_targets: Map<NodeId, DragPreview> | null = null;
	let queued_bezier_handle_targets: Map<NodeId, BezierEasingPreview> | null = null;
	let drag_commit_raf_id = 0;
	let view_bounds_animation_frame = 0;
	let view_bounds_animation_target: CurveViewBounds | null = null;
	let pending_param_write_promises: Set<Promise<void>> = new Set();
	let active_curve_draw_points = $derived.by(
		() =>
			active_curve_draw?.points.map((point) => ({
				position: point.position,
				value: point.value
			})) ?? []
	);

	const clamp = (value: number, min: number, max: number): number =>
		Math.min(max, Math.max(min, value));
	const finite_or = (value: number, fallback: number): number =>
		Number.isFinite(value) ? value : fallback;
	const param_number_value = (value: ParamValue | undefined, fallback: number): number => {
		if (!value) {
			return fallback;
		}
		if (value.kind === 'float') {
			return finite_or(value.value, fallback);
		}
		if (value.kind === 'int') {
			return finite_or(value.value, fallback);
		}
		return fallback;
	};
	const param_string_value = (value: ParamValue | undefined, fallback: string): string => {
		if (!value) {
			return fallback;
		}
		if (value.kind === 'enum') {
			return value.value.trim();
		}
		if (value.kind === 'str') {
			return value.value;
		}
		return fallback;
	};
	const normalize_easing_kind = (value: string): CurveEasingKind => {
		const normalized = value.trim().toLowerCase();
		if (normalized === 'elastic') {
			return 'elastic';
		}
		if (normalized === 'bounce') {
			return 'bounce';
		}
		if (normalized === 'bezier') {
			return 'bezier';
		}
		if (normalized === 'hold') {
			return 'hold';
		}
		if (normalized === 'steps') {
			return 'steps';
		}
		if (normalized === 'shape') {
			return 'shape';
		}
		if (normalized === 'perlinnoise') {
			return 'perlinNoise';
		}
		if (normalized === 'random') {
			return 'random';
		}
		if (normalized === 'script') {
			return 'script';
		}
		return 'linear';
	};
	const normalize_step_mode = (value: string): CurveStepMode =>
		value.trim().toLowerCase() === 'stepsize' ? 'stepSize' : 'numSteps';
	const normalize_shape = (value: string): CurveShape => {
		const normalized = value.trim().toLowerCase();
		if (normalized === 'triangle') {
			return 'triangle';
		}
		if (normalized === 'saw') {
			return 'saw';
		}
		if (normalized === 'reversesaw') {
			return 'reverseSaw';
		}
		if (normalized === 'square') {
			return 'square';
		}
		return 'sine';
	};
	const normalize_phase_mode = (value: string): CurvePhaseMode =>
		value.trim().toLowerCase() === 'numphases' ? 'numPhases' : 'frequency';
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
	const easing_kind_options_for_key = (key: ParsedCurveKey | undefined): EasingKindOption[] => {
		const kind_param = key?.easing?.params[DECL_KIND];
		const enum_options = kind_param?.constraints.enum_options ?? [];
		if (enum_options.length === 0) {
			return DEFAULT_EASING_KIND_ORDER.map((value) => ({
				value,
				label: EASING_KIND_LABELS[value]
			}));
		}

		const options: EasingKindOption[] = [];
		const seen = new Set<CurveEasingKind>();
		for (const option of enum_options) {
			const value = normalize_easing_kind(param_string_value(option.value, option.variant_id));
			if (seen.has(value)) {
				continue;
			}
			seen.add(value);
			options.push({
				value,
				label: option.label.trim().length > 0 ? option.label : EASING_KIND_LABELS[value]
			});
		}
		return options.length > 0
			? options
			: DEFAULT_EASING_KIND_ORDER.map((value) => ({
					value,
					label: EASING_KIND_LABELS[value]
				}));
	};
	const normalize_bounds_pair = (
		min_candidate: number,
		max_candidate: number
	): { min: number; max: number } | null => {
		if (!Number.isFinite(min_candidate) || !Number.isFinite(max_candidate)) {
			return null;
		}
		const min = Math.min(min_candidate, max_candidate);
		const max = Math.max(min_candidate, max_candidate);
		if (max - min <= CURVE_EPSILON) {
			return null;
		}
		return { min, max };
	};
	const make_curve_range_constraint = (
		x_min_candidate: number,
		x_max_candidate: number,
		y_min_candidate: number,
		y_max_candidate: number
	): CurveRangeConstraint | null => {
		const x_bounds = normalize_bounds_pair(x_min_candidate, x_max_candidate);
		const y_bounds = normalize_bounds_pair(y_min_candidate, y_max_candidate);
		if (!x_bounds || !y_bounds) {
			return null;
		}
		return {
			x_min: x_bounds.min,
			x_max: x_bounds.max,
			y_min: y_bounds.min,
			y_max: y_bounds.max
		};
	};
	const range_bounds_from_value = (
		value: ParamValue | undefined
	): { min: number; max: number } | null => {
		if (!value || value.kind !== 'vec2') {
			return null;
		}
		return normalize_bounds_pair(value.value[0], value.value[1]);
	};
	const uniform_bounds_from_constraints = (
		constraints: UiParamConstraints | undefined
	): { min: number; max: number } | null => {
		const range = constraints?.range;
		if (!range || range.kind !== 'uniform') {
			return null;
		}
		if (range.min === undefined || range.max === undefined) {
			return null;
		}
		return normalize_bounds_pair(range.min, range.max);
	};

	const cubic_from_points = (p0: number, p1: number, p2: number, p3: number): CubicPolynomial => ({
		a: -p0 + 3 * p1 - 3 * p2 + p3,
		b: 3 * p0 - 6 * p1 + 3 * p2,
		c: -3 * p0 + 3 * p1,
		d: p0
	});
	const sample_cubic = (poly: CubicPolynomial, t: number): number =>
		((poly.a * t + poly.b) * t + poly.c) * t + poly.d;
	const derivative_cubic = (poly: CubicPolynomial, t: number): number =>
		(3 * poly.a * t + 2 * poly.b) * t + poly.c;

	const smoothstep = (value: number): number => value * value * (3 - 2 * value);
	const stable_mix_u32 = (value: number): number => {
		let mixed = value >>> 0;
		mixed ^= mixed >>> 16;
		mixed = Math.imul(mixed, 0x7feb352d);
		mixed ^= mixed >>> 15;
		mixed = Math.imul(mixed, 0x846ca68b);
		mixed ^= mixed >>> 16;
		return mixed >>> 0;
	};
	const hash_i32 = (value: number, seed: number): number => {
		const base = (value | 0) + Math.imul(seed | 0, 0x9e3779b9);
		return stable_mix_u32(base);
	};
	const hash_to_unit = (hash: number): number => (hash >>> 0) / 0xffffffff;
	const value_noise_1d = (position: number, seed: number): number => {
		const left = Math.floor(position);
		const right = left + 1;
		const local = clamp(position - left, 0, 1);
		const weight = smoothstep(local);
		const left_value = hash_to_unit(hash_i32(left, seed)) * 2 - 1;
		const right_value = hash_to_unit(hash_i32(right, seed)) * 2 - 1;
		return left_value + (right_value - left_value) * weight;
	};
	const fractal_noise_1d = (position: number, octaves: number, seed: number): number => {
		if (!Number.isFinite(position)) {
			return 0;
		}

		let frequency = 1;
		let amplitude = 1;
		let total = 0;
		let normalization = 0;
		const octave_count = Math.max(1, Math.min(MAX_PERLIN_OCTAVES, Math.round(octaves)));
		for (let octave = 0; octave < octave_count; octave += 1) {
			const octave_seed = stable_mix_u32(seed ^ Math.imul(octave + 1, 0x9e3779b9));
			total += value_noise_1d(position * frequency, octave_seed) * amplitude;
			normalization += amplitude;
			frequency *= 2;
			amplitude *= 0.5;
		}
		if (normalization <= CURVE_EPSILON) {
			return 0;
		}
		return total / normalization;
	};
	const fade_envelope = (progress: number, fade_in: number, fade_out: number): number => {
		let envelope = 1;
		if (fade_in > CURVE_EPSILON) {
			envelope = Math.min(envelope, clamp(progress / fade_in, 0, 1));
		}
		if (fade_out > CURVE_EPSILON) {
			envelope = Math.min(envelope, clamp((1 - progress) / fade_out, 0, 1));
		}
		return clamp(envelope, 0, 1);
	};
	const elastic_progress = (progress: number): number => {
		if (progress <= 0) {
			return 0;
		}
		if (progress >= 1) {
			return 1;
		}
		const oscillation = (progress * 10 - 0.75) * ((Math.PI * 2) / 3);
		return Math.pow(2, -10 * progress) * Math.sin(oscillation) + 1;
	};
	const bounce_progress = (progress: number): number => {
		if (progress <= 0) {
			return 0;
		}
		if (progress >= 1) {
			return 1;
		}

		let t = progress;
		const n1 = 7.5625;
		const d1 = 2.75;
		if (t < 1 / d1) {
			return n1 * t * t;
		}
		if (t < 2 / d1) {
			t -= 1.5 / d1;
			return n1 * t * t + 0.75;
		}
		if (t < 2.5 / d1) {
			t -= 2.25 / d1;
			return n1 * t * t + 0.9375;
		}
		t -= 2.625 / d1;
		return n1 * t * t + 0.984375;
	};
	const shape_wave = (shape: CurveShape, phase_cycles: number): number => {
		const phase = ((phase_cycles % 1) + 1) % 1;
		if (shape === 'triangle') {
			return 1 - 4 * Math.abs(phase - 0.5);
		}
		if (shape === 'saw') {
			return 2 * phase - 1;
		}
		if (shape === 'reverseSaw') {
			return 1 - 2 * phase;
		}
		if (shape === 'square') {
			return phase < 0.5 ? 1 : -1;
		}
		return Math.sin(phase * Math.PI * 2);
	};

	const to_param_ref = (candidate: UiNodeDto | undefined): ParamNodeRef | undefined => {
		if (!candidate || candidate.data.kind !== 'parameter') {
			return undefined;
		}
		return {
			node_id: candidate.node_id,
			value: candidate.data.param.value,
			enabled: candidate.meta.enabled,
			constraints: candidate.data.param.constraints
		};
	};

	const compile_curve = (parsed_keys: ParsedCurveKey[]): CompiledCurve => {
		const normalized: CompiledCurveKey[] = parsed_keys
			.filter((key) => Number.isFinite(key.position) && Number.isFinite(key.value))
			.map((key) => ({
				node_id: key.node_id,
				position: key.position,
				value: key.value,
				easing: key.easing
			}))
			.sort((left, right) => left.position - right.position || left.node_id - right.node_id);

		if (normalized.length > 1) {
			const deduplicated: CompiledCurveKey[] = [];
			for (const key of normalized) {
				const previous = deduplicated[deduplicated.length - 1];
				if (previous && Math.abs(previous.position - key.position) <= CURVE_EPSILON) {
					deduplicated[deduplicated.length - 1] = key;
					continue;
				}
				deduplicated.push(key);
			}
			normalized.splice(0, normalized.length, ...deduplicated);
		}

		const segments: CompiledCurveSegment[] = [];
		for (let index = 0; index + 1 < normalized.length; index += 1) {
			const start = normalized[index];
			const end = normalized[index + 1];
			const span = end.position - start.position;
			if (span <= CURVE_EPSILON) {
				continue;
			}

			const params = start.easing?.params ?? {};
			const easing_kind =
				start.easing?.kind ??
				normalize_easing_kind(param_string_value(params[DECL_KIND]?.value, 'linear'));

			let easing: CompiledSegmentEasing = { kind: 'linear' };
			if (easing_kind === 'elastic') {
				easing = { kind: 'elastic' };
			} else if (easing_kind === 'bounce') {
				easing = { kind: 'bounce' };
			} else if (easing_kind === 'hold') {
				easing = { kind: 'hold' };
			} else if (easing_kind === 'bezier') {
				const out_position = param_number_value(params[DECL_OUT_POSITION]?.value, 1 / 3);
				const out_value = param_number_value(params[DECL_OUT_VALUE]?.value, 1 / 3);
				const in_position = param_number_value(params[DECL_IN_POSITION]?.value, -1 / 3);
				const in_value = param_number_value(params[DECL_IN_VALUE]?.value, -1 / 3);

				const value_span = end.value - start.value;
				let p1x = finite_or(start.position + out_position * span, start.position + span / 3);
				let p2x = finite_or(end.position + in_position * span, start.position + (span * 2) / 3);
				const p1y = finite_or(start.value + out_value, start.value + value_span / 3);
				const p2y = finite_or(end.value + in_value, start.value + (value_span * 2) / 3);
				p1x = clamp(p1x, start.position, end.position);
				p2x = clamp(p2x, start.position, end.position);
				easing = {
					kind: 'bezier',
					x: cubic_from_points(start.position, p1x, p2x, end.position),
					y: cubic_from_points(start.value, p1y, p2y, end.value),
					start_position: start.position,
					inv_span: 1 / span
				};
			} else if (easing_kind === 'steps') {
				const step_mode = normalize_step_mode(
					param_string_value(params[DECL_STEP_MODE]?.value, 'numSteps')
				);
				let steps = Math.max(1, param_number_value(params[DECL_NUM_STEPS]?.value, 8));
				if (step_mode === 'stepSize') {
					const step_size = Math.max(
						CURVE_EPSILON,
						Math.abs(param_number_value(params[DECL_STEP_SIZE]?.value, 0.1))
					);
					steps = Math.max(1, Math.ceil(Math.abs(span) / step_size));
				}
				easing = {
					kind: 'steps',
					steps
				};
			} else if (easing_kind === 'shape') {
				const phase_mode = normalize_phase_mode(
					param_string_value(params[DECL_PHASE_MODE]?.value, 'frequency')
				);
				const shape = normalize_shape(param_string_value(params[DECL_SHAPE]?.value, 'sine'));
				const amplitude = param_number_value(params[DECL_AMPLITUDE]?.value, 1);
				const frequency = Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1));
				const num_phases = Math.abs(param_number_value(params[DECL_NUM_PHASES]?.value, 1));
				const phase_cycles = phase_mode === 'frequency' ? frequency * Math.abs(span) : num_phases;
				easing = {
					kind: 'shape',
					shape,
					amplitude,
					phase_cycles,
					fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
					fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0))
				};
			} else if (easing_kind === 'perlinNoise') {
				easing = {
					kind: 'perlinNoise',
					frequency: Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 1)),
					amplitude: param_number_value(params[DECL_AMPLITUDE]?.value, 1),
					octaves: clamp(param_number_value(params[DECL_OCTAVES]?.value, 4), 1, MAX_PERLIN_OCTAVES),
					fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
					fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0)),
					phase: param_number_value(params[DECL_PHASE]?.value, 0),
					seed: 0
				};
			} else if (easing_kind === 'random') {
				easing = {
					kind: 'random',
					frequency: Math.abs(param_number_value(params[DECL_FREQUENCY]?.value, 6)),
					fade_in: Math.max(0, param_number_value(params[DECL_FADE_IN]?.value, 0)),
					fade_out: Math.max(0, param_number_value(params[DECL_FADE_OUT]?.value, 0)),
					seed: Math.round(param_number_value(params[DECL_SEED]?.value, 0))
				};
			} else if (easing_kind === 'script') {
				easing = {
					kind: 'script',
					source: param_string_value(params[DECL_SCRIPT_SOURCE]?.value, '')
				};
			}

			segments.push({
				start_position: start.position,
				end_position: end.position,
				start_value: start.value,
				end_value: end.value,
				value_delta: end.value - start.value,
				inv_span: 1 / span,
				easing
			});
		}

		return {
			keys: normalized,
			segments
		};
	};

	const sample_bezier = (
		position: number,
		x: CubicPolynomial,
		y: CubicPolynomial,
		start_position: number,
		inv_span: number
	): number => {
		let t = clamp((position - start_position) * inv_span, 0, 1);
		let converged = false;
		for (let iteration = 0; iteration < 6; iteration += 1) {
			const sampled_x = sample_cubic(x, t);
			const delta = sampled_x - position;
			if (Math.abs(delta) <= 1e-10) {
				converged = true;
				break;
			}
			const derivative = derivative_cubic(x, t);
			if (Math.abs(derivative) <= CURVE_EPSILON) {
				break;
			}
			const next_t = t - delta / derivative;
			if (!Number.isFinite(next_t)) {
				break;
			}
			t = clamp(next_t, 0, 1);
		}

		if (!converged) {
			let low = 0;
			let high = 1;
			for (let iteration = 0; iteration < 20; iteration += 1) {
				const midpoint = (low + high) * 0.5;
				const sampled_x = sample_cubic(x, midpoint);
				if (sampled_x <= position) {
					low = midpoint;
				} else {
					high = midpoint;
				}
			}
			t = (low + high) * 0.5;
		}
		return sample_cubic(y, t);
	};

	const solve_bezier_t_for_position = (
		position: number,
		x: CubicPolynomial,
		start_position: number,
		inv_span: number
	): number => {
		let t = clamp((position - start_position) * inv_span, 0, 1);
		for (let iteration = 0; iteration < 6; iteration += 1) {
			const sampled_x = sample_cubic(x, t);
			const delta = sampled_x - position;
			if (Math.abs(delta) <= 1e-9) {
				break;
			}
			const derivative = derivative_cubic(x, t);
			if (Math.abs(derivative) <= CURVE_EPSILON) {
				break;
			}
			t = clamp(t - delta / derivative, 0, 1);
		}

		let low = 0;
		let high = 1;
		for (let iteration = 0; iteration < 14; iteration += 1) {
			const midpoint = (low + high) * 0.5;
			const sampled_x = sample_cubic(x, midpoint);
			if (sampled_x <= position) {
				low = midpoint;
			} else {
				high = midpoint;
			}
		}
		return clamp((low + high) * 0.5, 0, 1);
	};

	const sample_segment = (segment: CompiledCurveSegment, position: number): number => {
		if (position <= segment.start_position) {
			return segment.start_value;
		}
		if (position >= segment.end_position) {
			return segment.end_value;
		}

		const progress = clamp((position - segment.start_position) * segment.inv_span, 0, 1);
		const linear_value = segment.start_value + segment.value_delta * progress;
		const easing = segment.easing;

		if (easing.kind === 'linear') {
			return linear_value;
		}
		if (easing.kind === 'elastic') {
			return segment.start_value + segment.value_delta * elastic_progress(progress);
		}
		if (easing.kind === 'bounce') {
			return segment.start_value + segment.value_delta * bounce_progress(progress);
		}
		if (easing.kind === 'hold') {
			return segment.start_value;
		}
		if (easing.kind === 'bezier') {
			return sample_bezier(position, easing.x, easing.y, easing.start_position, easing.inv_span);
		}
		if (easing.kind === 'steps') {
			const steps = Math.max(1, Math.round(easing.steps));
			const scaled_progress = progress * steps;
			const rounded_progress = Math.round(scaled_progress);
			const snap_tolerance = Number.EPSILON * Math.max(32, Math.abs(scaled_progress) * 64);
			const step_index = clamp(
				Math.abs(scaled_progress - rounded_progress) <= snap_tolerance
					? rounded_progress
					: Math.floor(scaled_progress),
				0,
				steps
			);
			const stepped_progress = clamp(step_index / steps, 0, 1);
			return segment.start_value + segment.value_delta * stepped_progress;
		}
		if (easing.kind === 'shape') {
			const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
			const wave = shape_wave(easing.shape, easing.phase_cycles * progress);
			return linear_value + easing.amplitude * wave * envelope;
		}
		if (easing.kind === 'perlinNoise') {
			if (easing.frequency <= CURVE_EPSILON || Math.abs(easing.amplitude) <= CURVE_EPSILON) {
				return linear_value;
			}
			const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
			const noise_position = (position - segment.start_position) * easing.frequency + easing.phase;
			const noise = fractal_noise_1d(noise_position, easing.octaves, easing.seed);
			return linear_value + noise * easing.amplitude * envelope;
		}
		if (easing.kind === 'random') {
			if (easing.frequency <= CURVE_EPSILON) {
				return linear_value;
			}
			const envelope = fade_envelope(progress, easing.fade_in, easing.fade_out);
			const bucket = Math.floor((position - segment.start_position) * easing.frequency);
			const random_progress = hash_to_unit(hash_i32(bucket, easing.seed));
			const random_value = segment.start_value + segment.value_delta * random_progress;
			return linear_value + (random_value - linear_value) * envelope;
		}
		return linear_value;
	};

	const sample_compiled_curve = (
		compiled_curve: CompiledCurve,
		position: number
	): number | null => {
		const keys = compiled_curve.keys;
		if (keys.length === 0 || !Number.isFinite(position)) {
			return null;
		}
		if (keys.length === 1) {
			return keys[0].value;
		}
		const first = keys[0];
		const last = keys[keys.length - 1];
		if (position <= first.position) {
			return first.value;
		}
		if (position >= last.position) {
			return last.value;
		}

		const segments = compiled_curve.segments;
		if (segments.length === 0) {
			return first.value;
		}

		let low = 0;
		let high = segments.length;
		while (low < high) {
			const middle = (low + high) >> 1;
			if (segments[middle].end_position <= position) {
				low = middle + 1;
			} else {
				high = middle;
			}
		}
		const segment_index = Math.min(low, segments.length - 1);
		return sample_segment(segments[segment_index], position);
	};

	const coerce_param_value_for_ref = (ref: ParamNodeRef, value: ParamValue): ParamValue | null => {
		if (ref.value.kind === 'int') {
			if (value.kind === 'int') {
				return { kind: 'int', value: Math.round(value.value) };
			}
			if (value.kind === 'float') {
				return { kind: 'int', value: Math.round(value.value) };
			}
			return null;
		}
		if (ref.value.kind === 'float') {
			if (value.kind === 'int' || value.kind === 'float') {
				return { kind: 'float', value: value.value };
			}
			return null;
		}
		if (ref.value.kind === 'enum') {
			if (value.kind === 'enum' || value.kind === 'str') {
				return { kind: 'enum', value: value.value };
			}
			return null;
		}
		if (ref.value.kind === 'str') {
			if (value.kind === 'enum' || value.kind === 'str') {
				return { kind: 'str', value: value.value };
			}
			return null;
		}
		if (ref.value.kind === value.kind) {
			return value;
		}
		return null;
	};

	const set_param_value = async (
		ref: ParamNodeRef | undefined,
		value: ParamValue
	): Promise<void> => {
		const operation = (async (): Promise<void> => {
			if (!ref) {
				return;
			}
			const coerced = coerce_param_value_for_ref(ref, value);
			if (!coerced) {
				return;
			}
			if ((coerced.kind === 'float' || coerced.kind === 'int') && !Number.isFinite(coerced.value)) {
				return;
			}
			await sendSetParamIntent(ref.node_id, coerced, 'Coalesce');
		})();
		pending_param_write_promises.add(operation);
		void operation.finally(() => {
			pending_param_write_promises.delete(operation);
		});
		await operation;
	};

	const set_numeric_param = async (ref: ParamNodeRef | undefined, value: number): Promise<void> => {
		if (!Number.isFinite(value)) {
			return;
		}
		await set_param_value(ref, { kind: 'float', value });
	};

	const clamp_curve_position_to_active_range = (position: number): number => {
		const active_range = active_curve_range_constraint;
		if (!active_range || !Number.isFinite(position)) {
			return position;
		}
		return clamp(position, active_range.x_min, active_range.x_max);
	};

	const clamp_curve_value_to_active_range = (value: number): number => {
		const active_range = active_curve_range_constraint;
		if (!active_range || !Number.isFinite(value)) {
			return value;
		}
		return clamp(value, active_range.y_min, active_range.y_max);
	};

	const clamp_curve_point_to_active_range = (
		position: number,
		value: number
	): { position: number; value: number } => ({
		position: clamp_curve_position_to_active_range(position),
		value: clamp_curve_value_to_active_range(value)
	});

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

	const has_selected_key = (key_id: NodeId): boolean => selected_key_ids.includes(key_id);

	const clear_selected_keys = (): void => {
		if (selected_key_id !== null) {
			selected_key_id = null;
		}
		if (selected_key_ids.length > 0) {
			selected_key_ids = [];
		}
	};

	const clear_selected_curve_owner_keys = (): void => {
		if (selected_curve_owner_key_ids.length > 0) {
			selected_curve_owner_key_ids = [];
		}
	};

	const key_hit_threshold_px = (): number => Math.max(8, rem_base_px * KEY_HIT_RADIUS_REM);
	const curve_hit_threshold_px = (): number => Math.max(7, rem_base_px * CURVE_HIT_RADIUS_REM);
	const bezier_handle_hit_threshold_px = (): number =>
		Math.max(8, rem_base_px * BEZIER_HANDLE_HIT_RADIUS_REM);

	const set_single_selected_key = (key_id: NodeId | null): void => {
		if (key_id !== null) {
			clear_selected_curve_owner_keys();
		}
		selected_key_id = key_id;
		selected_key_ids = key_id === null ? [] : [key_id];
	};

	const add_selected_key = (key_id: NodeId): void => {
		clear_selected_curve_owner_keys();
		if (has_selected_key(key_id)) {
			if (selected_key_id === null) {
				selected_key_id = key_id;
			}
			return;
		}
		selected_key_ids = [...selected_key_ids, key_id];
		if (selected_key_id === null) {
			selected_key_id = key_id;
		}
	};

	const has_selected_curve_owner_key = (key_id: NodeId): boolean =>
		selected_curve_owner_key_ids.includes(key_id);

	const set_single_selected_curve_owner_key = (key_id: NodeId | null): void => {
		if (key_id !== null) {
			clear_selected_keys();
		}
		selected_curve_owner_key_ids = key_id === null ? [] : [key_id];
	};

	const add_selected_curve_owner_key = (key_id: NodeId): void => {
		clear_selected_keys();
		if (has_selected_curve_owner_key(key_id)) {
			return;
		}
		selected_curve_owner_key_ids = [...selected_curve_owner_key_ids, key_id];
	};

	const ordered_node_ids = (node_ids: Iterable<NodeId>): NodeId[] => {
		const selected = new Set(node_ids);
		const ordered: NodeId[] = [];
		for (const key of parsed_keys) {
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

	const set_drag_preview = (key_id: NodeId, position: number, value: number): void => {
		const clamped = clamp_curve_point_to_active_range(position, value);
		const existing = drag_preview_by_key_id.get(key_id);
		if (
			existing &&
			Math.abs(existing.position - clamped.position) <= CURVE_EPSILON &&
			Math.abs(existing.value - clamped.value) <= CURVE_EPSILON
		) {
			return;
		}
		const next = new Map(drag_preview_by_key_id);
		next.set(key_id, { position: clamped.position, value: clamped.value });
		drag_preview_by_key_id = next;
	};

	const clear_drag_preview = (key_id: NodeId): void => {
		if (!drag_preview_by_key_id.has(key_id)) {
			return;
		}
		const next = new Map(drag_preview_by_key_id);
		next.delete(key_id);
		drag_preview_by_key_id = next;
	};

	const set_easing_drag_preview = (key_id: NodeId, preview: BezierEasingPreview): void => {
		const existing = easing_drag_preview_by_key_id.get(key_id);
		if (
			existing &&
			Math.abs(existing.out_position - preview.out_position) <= CURVE_EPSILON &&
			Math.abs(existing.out_value - preview.out_value) <= CURVE_EPSILON &&
			Math.abs(existing.in_position - preview.in_position) <= CURVE_EPSILON &&
			Math.abs(existing.in_value - preview.in_value) <= CURVE_EPSILON
		) {
			return;
		}
		const next = new Map(easing_drag_preview_by_key_id);
		next.set(key_id, preview);
		easing_drag_preview_by_key_id = next;
	};

	const clear_easing_drag_preview = (key_id: NodeId): void => {
		if (!easing_drag_preview_by_key_id.has(key_id)) {
			return;
		}
		const next = new Map(easing_drag_preview_by_key_id);
		next.delete(key_id);
		easing_drag_preview_by_key_id = next;
	};

	const clear_drag_previews_if_unchanged = (snapshots: Map<NodeId, DragPreview>): void => {
		if (snapshots.size === 0 || drag_preview_by_key_id.size === 0) {
			return;
		}
		const current = drag_preview_by_key_id;
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
			drag_preview_by_key_id = next;
		}
	};

	const clear_easing_drag_previews_if_unchanged = (
		snapshots: Map<NodeId, BezierEasingPreview>
	): void => {
		if (snapshots.size === 0 || easing_drag_preview_by_key_id.size === 0) {
			return;
		}
		const current = easing_drag_preview_by_key_id;
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
			easing_drag_preview_by_key_id = next;
		}
	};

	const HANDLE_ALIGNMENT_COSINE_TOLERANCE = 0.035;
	const KEY_NEIGHBOR_POSITION_GAP = 1e-8;

	const build_key_drag_position_delta_constraint = (
		origins: Array<{ key_id: NodeId; position: number; value: number }>
	): KeyDragPositionConstraint | null => {
		if (origins.length === 0 || parsed_keys.length === 0) {
			return null;
		}

		const selected_key_set = new Set<NodeId>(origins.map((entry) => entry.key_id));
		const key_index_by_id = new Map<NodeId, number>();
		for (let index = 0; index < parsed_keys.length; index += 1) {
			key_index_by_id.set(parsed_keys[index].node_id, index);
		}

		let min_delta_position = Number.NEGATIVE_INFINITY;
		let max_delta_position = Number.POSITIVE_INFINITY;
		let has_constraint = false;
		const active_range = active_curve_range_constraint;

		for (const origin of origins) {
			const index = key_index_by_id.get(origin.key_id);
			if (index === undefined) {
				continue;
			}
			const previous = index > 0 ? parsed_keys[index - 1] : null;
			if (previous && !selected_key_set.has(previous.node_id)) {
				min_delta_position = Math.max(
					min_delta_position,
					previous.position + KEY_NEIGHBOR_POSITION_GAP - origin.position
				);
				has_constraint = true;
			}
			const next = index + 1 < parsed_keys.length ? parsed_keys[index + 1] : null;
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
		if (drag_key_ids.length === 0 || parsed_keys.length < 3) {
			return [];
		}
		const index_by_key_id = new Map<NodeId, number>();
		for (let index = 0; index < parsed_keys.length; index += 1) {
			index_by_key_id.set(parsed_keys[index].node_id, index);
		}

		const constraints: KeyDragBezierAlignmentConstraint[] = [];
		const unique_drag_key_ids = new Set(drag_key_ids);
		for (const key_id of unique_drag_key_ids) {
			const index = index_by_key_id.get(key_id);
			if (index === undefined || index <= 0 || index >= parsed_keys.length - 1) {
				continue;
			}
			const previous = parsed_keys[index - 1];
			const anchor = parsed_keys[index];
			const next = parsed_keys[index + 1];
			const left_segment = resolve_bezier_handle_segment(previous, anchor);
			const right_segment = resolve_bezier_handle_segment(anchor, next);
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

	const override_numeric_param_ref = (
		ref: ParamNodeRef | undefined,
		value: number | undefined
	): ParamNodeRef | undefined => {
		if (!ref || value === undefined || !Number.isFinite(value)) {
			return ref;
		}
		if (ref.value.kind === 'int') {
			return {
				...ref,
				value: { kind: 'int', value: Math.round(value) }
			};
		}
		if (ref.value.kind === 'float') {
			return {
				...ref,
				value: { kind: 'float', value }
			};
		}
		return ref;
	};

	let parsed_keys = $derived.by((): ParsedCurveKey[] => {
		const nodes_by_id = graphNodesById;
		if (!nodes_by_id || liveNode.node_type !== CURVE_NODE_TYPE) {
			return [];
		}

		const parsed: ParsedCurveKey[] = [];
		for (let source_index = 0; source_index < liveNode.children.length; source_index += 1) {
			const child_id = liveNode.children[source_index];
			const key_node = nodes_by_id.get(child_id);
			if (!key_node || key_node.node_type !== KEY_NODE_TYPE) {
				continue;
			}

			let position_param: ParamNodeRef | undefined;
			let value_param: ParamNodeRef | undefined;
			let easing: ParsedEasing | undefined;

			for (const key_child_id of key_node.children) {
				const key_child = nodes_by_id.get(key_child_id);
				if (!key_child) {
					continue;
				}
				const decl_id = key_child.decl_id;
				if (decl_id === DECL_POSITION) {
					position_param = to_param_ref(key_child);
					continue;
				}
				if (decl_id === DECL_VALUE) {
					value_param = to_param_ref(key_child);
					continue;
				}
				if (decl_id === DECL_EASING && key_child.node_type === EASING_NODE_TYPE) {
					const easing_params: Record<string, ParamNodeRef> = {};
					for (const easing_child_id of key_child.children) {
						const easing_child = nodes_by_id.get(easing_child_id);
						const param_ref = to_param_ref(easing_child);
						if (!easing_child || !param_ref) {
							continue;
						}
						easing_params[easing_child.decl_id] = param_ref;
					}
					easing = {
						node_id: key_child.node_id,
						kind: normalize_easing_kind(
							param_string_value(easing_params[DECL_KIND]?.value, 'linear')
						),
						params: easing_params
					};
				}
			}

			const easing_preview = easing_drag_preview_by_key_id.get(key_node.node_id);
			if (easing && easing.kind === 'bezier' && easing_preview) {
				const next_params: Record<string, ParamNodeRef> = { ...easing.params };
				const next_out_position = override_numeric_param_ref(
					easing.params[DECL_OUT_POSITION],
					easing_preview.out_position
				);
				if (next_out_position) {
					next_params[DECL_OUT_POSITION] = next_out_position;
				}
				const next_out_value = override_numeric_param_ref(
					easing.params[DECL_OUT_VALUE],
					easing_preview.out_value
				);
				if (next_out_value) {
					next_params[DECL_OUT_VALUE] = next_out_value;
				}
				const next_in_position = override_numeric_param_ref(
					easing.params[DECL_IN_POSITION],
					easing_preview.in_position
				);
				if (next_in_position) {
					next_params[DECL_IN_POSITION] = next_in_position;
				}
				const next_in_value = override_numeric_param_ref(
					easing.params[DECL_IN_VALUE],
					easing_preview.in_value
				);
				if (next_in_value) {
					next_params[DECL_IN_VALUE] = next_in_value;
				}
				easing = {
					...easing,
					params: next_params
				};
			}

			let position = param_number_value(position_param?.value, 0);
			let value = param_number_value(value_param?.value, 0);
			const drag_preview = drag_preview_by_key_id.get(key_node.node_id);
			if (drag_preview) {
				position = drag_preview.position;
				value = drag_preview.value;
			}

			parsed.push({
				node_id: key_node.node_id,
				position,
				value,
				source_index,
				position_param,
				value_param,
				easing
			});
		}

		parsed.sort(
			(left, right) => left.position - right.position || left.source_index - right.source_index
		);
		return parsed;
	});

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
		if (selected_key_ids.length > 0) {
			return selected_key_ids;
		}
		if (selected_key_id !== null) {
			return [selected_key_id];
		}
		if (selected_curve_owner_key_ids.length > 0) {
			return selected_curve_owner_key_ids;
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
		const show_neighbors_on_both_sides = selected_key_ids.length > 0 || selected_key_id !== null;
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
				const aligned = vectors_are_aligned_and_opposite(
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
		const nodes_by_id = graphNodesById;
		if (!nodes_by_id || liveNode.node_type !== CURVE_NODE_TYPE) {
			return null;
		}
		for (const child_id of liveNode.children) {
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
			const nodes_by_id = graphNodesById;
			if (!nodes_by_id) {
				return null;
			}
			let x_bounds: { min: number; max: number } | null = null;
			let y_bounds: { min: number; max: number } | null = null;
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

		let x_bounds: { min: number; max: number } | null = null;
		let y_bounds: { min: number; max: number } | null = null;
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
		if (selected_key_id === null) {
			return null;
		}
		return parsed_key_by_id.get(selected_key_id) ?? null;
	});
	let key_creatable_item = $derived.by(
		() =>
			liveNode.creatable_user_items.find(
				(item) => item.node_type === KEY_NODE_TYPE || item.item_kind === KEY_ITEM_KIND
			) ?? null
	);
	let compiled_curve = $derived.by(() => compile_curve(parsed_keys));

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
	): { start: ParsedCurveKey; end: ParsedCurveKey } | null => {
		if (!Number.isFinite(position) || parsed_keys.length < 2) {
			return null;
		}
		let fallback: { start: ParsedCurveKey; end: ParsedCurveKey } | null = null;
		for (let index = 0; index + 1 < parsed_keys.length; index += 1) {
			const start = parsed_keys[index];
			const end = parsed_keys[index + 1];
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
			const segment = resolve_bezier_handle_segment(start, end);
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
			const start_alignment = resolve_curve_drag_opposite_alignment(start.node_id, {
				key_id: start.node_id,
				kind: 'out'
			});
			if (start_alignment) {
				apply_curve_drag_opposite_alignment(
					preserved_aligned_handles,
					start_alignment,
					point_a_x,
					point_a_y,
					start.position,
					start.value
				);
			}
			const end_alignment = resolve_curve_drag_opposite_alignment(end.node_id, {
				key_id: start.node_id,
				kind: 'in'
			});
			if (end_alignment) {
				apply_curve_drag_opposite_alignment(
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

	const apply_easing_param_values = async (
		key: ParsedCurveKey | undefined,
		values: Record<string, ParamValue>
	): Promise<void> => {
		if (!key?.easing || Object.keys(values).length === 0) {
			return;
		}
		const params = key.easing.params;
		const next_kind = values[DECL_KIND];
		if (next_kind) {
			await set_param_value(params[DECL_KIND], next_kind);
		}
		for (const [decl_id, value] of Object.entries(values)) {
			if (decl_id === DECL_KIND) {
				continue;
			}
			await set_param_value(params[decl_id], value);
		}
	};

	const easing_target_key_ids = (target: CurveContextMenuTarget | null): NodeId[] => {
		if (!target) {
			return [];
		}
		if (target.kind === 'all') {
			return compiled_curve.keys
				.slice(0, Math.max(0, compiled_curve.keys.length - 1))
				.map((key) => key.node_id);
		}
		return parsed_key_by_id.has(target.key_id) ? [target.key_id] : [];
	};

	const shared_easing_kind_for_key_ids = (key_ids: readonly NodeId[]): CurveEasingKind | null => {
		let shared_kind: CurveEasingKind | null = null;
		for (const key_id of key_ids) {
			const kind = parsed_key_by_id.get(key_id)?.easing?.kind ?? null;
			if (kind === null) {
				return null;
			}
			if (shared_kind === null) {
				shared_kind = kind;
				continue;
			}
			if (shared_kind !== kind) {
				return null;
			}
		}
		return shared_kind;
	};

	const apply_easing_kind_to_key_ids = async (
		key_ids: readonly NodeId[],
		easing_kind: CurveEasingKind,
		label: string
	): Promise<boolean> => {
		const ordered_key_ids = ordered_node_ids(key_ids);
		const target_keys = ordered_key_ids
			.map((key_id) => parsed_key_by_id.get(key_id))
			.filter(
				(key): key is ParsedCurveKey =>
					key !== undefined &&
					key.easing !== undefined &&
					key.easing.params[DECL_KIND] !== undefined &&
					key.easing.kind !== easing_kind
			);
		if (target_keys.length === 0) {
			return false;
		}

		const edit_session = createUiEditSession(label, 'curve-easing-kind');
		await edit_session.begin();
		try {
			for (const key of target_keys) {
				await apply_easing_param_values(key, {
					[DECL_KIND]: enum_param_value(easing_kind)
				});
			}
			return true;
		} finally {
			await edit_session.end();
		}
	};

	const cycle_easing_kind = async (key_id: NodeId, direction: 1 | -1): Promise<boolean> => {
		const key = parsed_key_by_id.get(key_id);
		if (!key?.easing || !key.easing.params[DECL_KIND]) {
			return false;
		}
		const options = easing_kind_options_for_key(key);
		if (options.length === 0) {
			return false;
		}
		let current_index = options.findIndex((option) => option.value === key.easing?.kind);
		if (current_index < 0) {
			current_index = 0;
		}
		const next_index = (current_index + direction + options.length) % Math.max(1, options.length);
		const next_kind = options[next_index]?.value;
		if (!next_kind) {
			return false;
		}
		return apply_easing_kind_to_key_ids([key_id], next_kind, 'Change Curve Easing');
	};

	const open_curve_context_menu = (
		target: CurveContextMenuTarget,
		screen_x: number,
		screen_y: number
	): void => {
		curve_context_menu_target = target;
		curve_context_menu_anchor = {
			kind: 'point',
			x: screen_x,
			y: screen_y
		};
		curve_context_menu_open = easing_target_key_ids(target).length > 0;
	};

	const close_curve_context_menu = (): void => {
		curve_context_menu_open = false;
		curve_context_menu_anchor = null;
		curve_context_menu_target = null;
	};

	let curve_context_menu_items = $derived.by((): ContextMenuItem[] => {
		const target = curve_context_menu_target;
		if (!target) {
			return [];
		}
		const key_ids = easing_target_key_ids(target);
		if (key_ids.length === 0) {
			return [];
		}
		const options = easing_kind_options_for_key(parsed_key_by_id.get(key_ids[0]));
		if (options.length === 0) {
			return [];
		}
		const shared_kind = shared_easing_kind_for_key_ids(key_ids);
		const action_label = target.kind === 'all' ? 'Change All Curve Easings' : 'Change Curve Easing';
		return [
			{
				id: target.kind === 'all' ? 'change-all-easings' : 'change-easing',
				label: target.kind === 'all' ? 'Change All Easings' : 'Change Easing',
				submenu: options.map((option) => ({
					id: `curve-easing-${target.kind}-${option.value}`,
					label: option.label,
					hint: shared_kind === option.value ? 'Current' : undefined,
					action: () => {
						void apply_easing_kind_to_key_ids(key_ids, option.value, action_label);
					}
				}))
			}
		];
	});

	const resolve_pending_create_key = (pending: PendingCreateTarget): ParsedCurveKey | null => {
		if (pending.created_key_id !== null) {
			return parsed_key_by_id.get(pending.created_key_id) ?? null;
		}
		for (const key of parsed_keys) {
			if (pending.known_key_ids.has(key.node_id)) {
				continue;
			}
			return key;
		}
		return null;
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

	const sync_pending_create_previews = (
		pending: PendingCreateTarget,
		created_key_id: NodeId
	): void => {
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

	const clear_pending_create_previews = (
		pending: PendingCreateTarget,
		created_key_id: NodeId
	): void => {
		clear_drag_preview(created_key_id);

		const clear_if_bezier_preview = (
			key_id: NodeId,
			values: Record<string, ParamValue> | null | undefined
		): void => {
			if (!bezier_preview_from_param_values(values)) {
				return;
			}
			clear_easing_drag_preview(key_id);
		};

		if (pending.split_easing) {
			clear_if_bezier_preview(
				pending.split_easing.source_start_key_id,
				pending.split_easing.left_param_values
			);
			clear_if_bezier_preview(created_key_id, pending.split_easing.right_param_values);
			for (const update of pending.split_easing.preserved_aligned_handle_updates ?? []) {
				clear_if_bezier_preview(update.key_id, update.values);
			}
		} else {
			clear_if_bezier_preview(created_key_id, pending.easing_values);
		}

		for (const update of pending.existing_easing_updates) {
			clear_if_bezier_preview(update.key_id, update.values);
		}
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
		pending: PendingCreateTarget,
		created_key: ParsedCurveKey
	): boolean => {
		if (!created_key.position_param || !created_key.value_param) {
			return false;
		}
		if (pending.split_easing) {
			const source_key = parsed_key_by_id.get(pending.split_easing.source_start_key_id);
			if (!has_easing_param_targets(source_key, pending.split_easing.left_param_values)) {
				return false;
			}
			if (!has_easing_param_targets(created_key, pending.split_easing.right_param_values)) {
				return false;
			}
			for (const update of pending.split_easing.preserved_aligned_handle_updates ?? []) {
				if (!has_easing_param_targets(parsed_key_by_id.get(update.key_id), update.values)) {
					return false;
				}
			}
		}
		if (!has_easing_param_targets(created_key, pending.easing_values)) {
			return false;
		}
		for (const update of pending.existing_easing_updates) {
			if (!has_easing_param_targets(parsed_key_by_id.get(update.key_id), update.values)) {
				return false;
			}
		}
		return true;
	};

	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}
		const refresh = (): void => {
			const parsed = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize);
			if (Number.isFinite(parsed) && parsed > 0) {
				rem_base_px = parsed;
			}
		};
		refresh();
		window.addEventListener('resize', refresh);
		return () => {
			window.removeEventListener('resize', refresh);
		};
	});

	$effect(() => {
		if (parsed_keys.length === 0) {
			if (selected_key_id !== null) {
				selected_key_id = null;
			}
			if (selected_key_ids.length > 0) {
				selected_key_ids = [];
			}
			return;
		}
		const available_key_ids = new Set(parsed_keys.map((entry) => entry.node_id));
		const filtered_selection = selected_key_ids.filter((key_id) => available_key_ids.has(key_id));
		if (filtered_selection.length === 0) {
			if (selected_key_id !== null && available_key_ids.has(selected_key_id)) {
				const singleton = [selected_key_id];
				if (!same_node_id_array(selected_key_ids, singleton)) {
					selected_key_ids = singleton;
				}
				return;
			}
			if (selected_key_id !== null) {
				selected_key_id = null;
			}
			if (selected_key_ids.length > 0) {
				selected_key_ids = [];
			}
			return;
		}
		if (!same_node_id_array(filtered_selection, selected_key_ids)) {
			selected_key_ids = filtered_selection;
		}
		if (
			(selected_key_id === null || !filtered_selection.includes(selected_key_id)) &&
			selected_key_id !== filtered_selection[0]
		) {
			selected_key_id = filtered_selection[0];
		}
	});

	$effect(() => {
		if (parsed_keys.length === 0) {
			if (selected_curve_owner_key_ids.length > 0) {
				selected_curve_owner_key_ids = [];
			}
			if (hover_curve_owner_key_id !== null) {
				hover_curve_owner_key_id = null;
			}
			return;
		}
		const available_key_ids = new Set(parsed_keys.map((entry) => entry.node_id));
		const filtered_selection = selected_curve_owner_key_ids.filter((key_id) =>
			available_key_ids.has(key_id)
		);
		if (!same_node_id_array(filtered_selection, selected_curve_owner_key_ids)) {
			selected_curve_owner_key_ids = filtered_selection;
		}
		if (hover_curve_owner_key_id !== null && !available_key_ids.has(hover_curve_owner_key_id)) {
			hover_curve_owner_key_id = null;
		}
	});

	$effect.pre(() => {
		const pending = pending_create_target;
		if (!pending) {
			return;
		}
		const created_key = resolve_pending_create_key(pending);
		if (!created_key) {
			return;
		}
		if (pending.created_key_id !== created_key.node_id) {
			pending_create_target = {
				...pending,
				created_key_id: created_key.node_id
			};
		}
		if (
			selected_key_id !== created_key.node_id ||
			selected_key_ids.length !== 1 ||
			selected_key_ids[0] !== created_key.node_id
		) {
			set_single_selected_key(created_key.node_id);
		}
		sync_pending_create_previews(pending, created_key.node_id);
	});

	$effect(() => {
		const pending = pending_create_target;
		if (!pending) {
			return;
		}
		const created_key = resolve_pending_create_key(pending);
		if (!created_key || !pending_create_targets_ready(pending, created_key)) {
			return;
		}
		pending_create_target = null;
		void (async () => {
			try {
				await set_numeric_param(created_key.position_param, pending.position);
				await set_numeric_param(created_key.value_param, pending.value);
				if (pending.split_easing) {
					await apply_easing_param_values(
						parsed_key_by_id.get(pending.split_easing.source_start_key_id),
						pending.split_easing.left_param_values
					);
					await apply_easing_param_values(created_key, pending.split_easing.right_param_values);
					for (const update of pending.split_easing.preserved_aligned_handle_updates ?? []) {
						await apply_easing_param_values(parsed_key_by_id.get(update.key_id), update.values);
					}
				} else if (pending.easing_values) {
					await apply_easing_param_values(created_key, pending.easing_values);
				}
				for (const update of pending.existing_easing_updates) {
					await apply_easing_param_values(parsed_key_by_id.get(update.key_id), update.values);
				}
			} finally {
				clear_pending_create_previews(pending, created_key.node_id);
				if (pending.edit_session) {
					await pending.edit_session.end();
				}
			}
		})();
	});

	const canvas_local_point = (
		event: MouseEvent | PointerEvent
	): { x: number; y: number } | null => {
		if (!canvas_element) {
			return null;
		}
		const rect = canvas_element.getBoundingClientRect();
		return {
			x: event.clientX - rect.left,
			y: event.clientY - rect.top
		};
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

	const screen_to_curve_point = (
		screen_x: number,
		screen_y: number
	): { position: number; value: number } | null => {
		if (!interaction_transform) {
			return null;
		}
		const normalized_x = clamp(
			(screen_x - interaction_transform.plot_left) / interaction_transform.plot_width,
			0,
			1
		);
		const normalized_y = clamp(
			(screen_y - interaction_transform.plot_top) / interaction_transform.plot_height,
			0,
			1
		);
		const curve_point = {
			position:
				interaction_transform.x_min +
				normalized_x * (interaction_transform.x_max - interaction_transform.x_min),
			value:
				interaction_transform.y_max -
				normalized_y * (interaction_transform.y_max - interaction_transform.y_min)
		};
		return clamp_curve_point_to_active_range(curve_point.position, curve_point.value);
	};

	const current_view_bounds = (): CurveViewBounds | null => {
		if (fixed_view_bounds) {
			return { ...fixed_view_bounds };
		}
		if (!interaction_transform) {
			return null;
		}
		return {
			x_min: interaction_transform.x_min,
			x_max: interaction_transform.x_max,
			y_min: interaction_transform.y_min,
			y_max: interaction_transform.y_max
		};
	};

	const padded_axis_bounds = (
		min_candidate: number,
		max_candidate: number,
		minimum_span_candidate: number,
		pad_ratio: number
	): { min: number; max: number } | null => {
		if (!Number.isFinite(min_candidate) || !Number.isFinite(max_candidate)) {
			return null;
		}
		const min = Math.min(min_candidate, max_candidate);
		const max = Math.max(min_candidate, max_candidate);
		const minimum_span = Math.max(
			MIN_VIEW_SPAN,
			Number.isFinite(minimum_span_candidate) ? minimum_span_candidate : MIN_VIEW_SPAN
		);
		const base_span = Math.max(max - min, minimum_span);
		const center = (min + max) * 0.5;
		const half_span = base_span * (0.5 + pad_ratio);
		return {
			min: center - half_span,
			max: center + half_span
		};
	};

	const selected_frame_key_ids = (): NodeId[] => {
		const selected_ids = selected_key_ids_ordered();
		if (selected_ids.length > 0) {
			return selected_ids;
		}
		if (selected_curve_owner_key_ids.length === 0) {
			return [];
		}
		const owner_set = new Set(selected_curve_owner_key_ids);
		const framed = new Set<NodeId>();
		for (let index = 0; index < parsed_keys.length; index += 1) {
			const key = parsed_keys[index];
			if (!owner_set.has(key.node_id)) {
				continue;
			}
			framed.add(key.node_id);
			const next = parsed_keys[index + 1];
			if (next) {
				framed.add(next.node_id);
			}
		}
		return parsed_keys.filter((entry) => framed.has(entry.node_id)).map((entry) => entry.node_id);
	};

	const estimate_frame_x_span = (keys: ParsedCurveKey[]): number => {
		if (keys.length >= 2) {
			return Math.max(MIN_VIEW_SPAN, keys[keys.length - 1].position - keys[0].position);
		}
		const focus = keys[0];
		if (!focus) {
			const active_range = active_curve_range_constraint;
			if (active_range) {
				return Math.max(MIN_VIEW_SPAN, active_range.x_max - active_range.x_min);
			}
			const current = current_view_bounds();
			if (current) {
				return Math.max(MIN_VIEW_SPAN, current.x_max - current.x_min);
			}
			return 1;
		}

		const index = parsed_keys.findIndex((entry) => entry.node_id === focus.node_id);
		const previous = index > 0 ? parsed_keys[index - 1] : null;
		const next = index >= 0 && index + 1 < parsed_keys.length ? parsed_keys[index + 1] : null;
		if (previous && next) {
			return Math.max(MIN_VIEW_SPAN, next.position - previous.position);
		}
		if (previous) {
			return Math.max(MIN_VIEW_SPAN, (focus.position - previous.position) * 2);
		}
		if (next) {
			return Math.max(MIN_VIEW_SPAN, (next.position - focus.position) * 2);
		}

		const active_range = active_curve_range_constraint;
		if (active_range) {
			return Math.max(MIN_VIEW_SPAN, (active_range.x_max - active_range.x_min) * 0.18);
		}
		const current = current_view_bounds();
		if (current) {
			return Math.max(MIN_VIEW_SPAN, (current.x_max - current.x_min) * 0.18);
		}
		return 1;
	};

	const sampled_curve_value_bounds = (
		x_min_candidate: number,
		x_max_candidate: number
	): { min: number; max: number } | null => {
		if (
			compiled_curve.keys.length === 0 ||
			!Number.isFinite(x_min_candidate) ||
			!Number.isFinite(x_max_candidate)
		) {
			return null;
		}
		const x_min = Math.min(x_min_candidate, x_max_candidate);
		const x_max = Math.max(x_min_candidate, x_max_candidate);
		const sample_count = x_max - x_min <= CURVE_EPSILON ? 1 : 96;
		let y_min = Number.POSITIVE_INFINITY;
		let y_max = Number.NEGATIVE_INFINITY;
		for (let index = 0; index < sample_count; index += 1) {
			const t = sample_count <= 1 ? 0 : index / (sample_count - 1);
			const position = sample_count <= 1 ? x_min : x_min + (x_max - x_min) * t;
			const sampled = sample_compiled_curve(compiled_curve, position);
			if (sampled === null) {
				continue;
			}
			const value = clamp_curve_value_to_active_range(sampled);
			if (value < y_min) {
				y_min = value;
			}
			if (value > y_max) {
				y_max = value;
			}
		}
		return Number.isFinite(y_min) && Number.isFinite(y_max) ? { min: y_min, max: y_max } : null;
	};

	const default_curve_view_bounds = (): CurveViewBounds | null => {
		const active_range = active_curve_range_constraint;
		if (active_range) {
			return {
				x_min: active_range.x_min,
				x_max: active_range.x_max,
				y_min: active_range.y_min,
				y_max: active_range.y_max
			};
		}
		if (compiled_curve.keys.length === 0) {
			return null;
		}

		let x_min = compiled_curve.keys[0]?.position ?? 0;
		let x_max = compiled_curve.keys[compiled_curve.keys.length - 1]?.position ?? x_min;
		if (!Number.isFinite(x_min) || !Number.isFinite(x_max)) {
			return null;
		}
		if (Math.abs(x_max - x_min) <= CURVE_EPSILON) {
			x_min -= 0.5;
			x_max += 0.5;
		}

		const sampled_bounds = sampled_curve_value_bounds(x_min, x_max);
		let y_min = sampled_bounds?.min ?? Number.POSITIVE_INFINITY;
		let y_max = sampled_bounds?.max ?? Number.NEGATIVE_INFINITY;
		for (const key of parsed_keys) {
			const value = clamp_curve_value_to_active_range(key.value);
			if (value < y_min) {
				y_min = value;
			}
			if (value > y_max) {
				y_max = value;
			}
		}
		if (!Number.isFinite(y_min) || !Number.isFinite(y_max)) {
			y_min = -1;
			y_max = 1;
		}
		if (Math.abs(y_max - y_min) <= CURVE_EPSILON) {
			const delta = Math.max(0.5, Math.abs(y_max) * 0.2);
			y_min -= delta;
			y_max += delta;
		}
		const value_pad = (y_max - y_min) * 0.08;
		return sanitize_view_bounds({
			x_min,
			x_max,
			y_min: y_min - value_pad,
			y_max: y_max + value_pad
		});
	};

	const curve_draw_bucket = (screen_x: number): number | null => {
		if (!interaction_transform) {
			return null;
		}
		return Math.round(
			clamp(screen_x - interaction_transform.plot_left, 0, interaction_transform.plot_width)
		);
	};

	const record_curve_draw_point = (
		active: ActiveCurveDraw,
		screen_x: number,
		curve_point: { position: number; value: number }
	): ActiveCurveDraw => {
		const bucket = curve_draw_bucket(screen_x);
		if (bucket === null) {
			return active;
		}
		const existing_index = active.path_buckets.lastIndexOf(bucket);
		const next_path_buckets =
			existing_index >= 0
				? active.path_buckets.slice(0, existing_index + 1)
				: [...active.path_buckets, bucket];
		const retained_buckets = new Set(next_path_buckets);
		const next_points = active.points.filter(
			(point) => retained_buckets.has(point.bucket) && point.bucket !== bucket
		);
		next_points.push({
			bucket,
			position: curve_point.position,
			value: curve_point.value
		});
		next_points.sort((left, right) => left.position - right.position);
		return {
			...active,
			points: next_points,
			path_buckets: next_path_buckets
		};
	};

	const curve_draw_fit_value_error = (): number => {
		if (interaction_transform) {
			const value_span = interaction_transform.y_max - interaction_transform.y_min;
			if (Number.isFinite(value_span) && value_span > CURVE_EPSILON) {
				return Math.max(
					MIN_VIEW_SPAN,
					(value_span / Math.max(1, interaction_transform.plot_height)) *
						CURVE_DRAW_FIT_MAX_ERROR_PX
				);
			}
		}
		const active_range = active_curve_range_constraint;
		if (active_range) {
			return Math.max(MIN_VIEW_SPAN, (active_range.y_max - active_range.y_min) * 0.01);
		}
		const current = current_view_bounds();
		if (current) {
			return Math.max(MIN_VIEW_SPAN, (current.y_max - current.y_min) * 0.01);
		}
		return 0.01;
	};

	const commit_curve_draw = async (points: RecordedCurveDrawPoint[]): Promise<void> => {
		if (points.length < 2) {
			return;
		}
		const edit_session = createUiEditSession('Draw Curve', 'curve-draw');
		await edit_session.begin();
		try {
			await sendFitAnimationCurvePathIntent(
				liveNode.node_id,
				points.map((point) => ({
					position: point.position,
					value: point.value
				})),
				{
					max_value_error: curve_draw_fit_value_error(),
					max_keys: CURVE_DRAW_FIT_MAX_KEYS
				}
			);
		} finally {
			await edit_session.end();
		}
	};

	const sanitize_view_bounds = (bounds: CurveViewBounds): CurveViewBounds | null => {
		if (
			!Number.isFinite(bounds.x_min) ||
			!Number.isFinite(bounds.x_max) ||
			!Number.isFinite(bounds.y_min) ||
			!Number.isFinite(bounds.y_max)
		) {
			return null;
		}

		const active_range = active_curve_range_constraint;
		if (active_range) {
			const range_span_x = active_range.x_max - active_range.x_min;
			const range_span_y = active_range.y_max - active_range.y_min;
			if (range_span_x <= CURVE_EPSILON || range_span_y <= CURVE_EPSILON) {
				return null;
			}

			let x_span = bounds.x_max - bounds.x_min;
			let y_span = bounds.y_max - bounds.y_min;
			if (!Number.isFinite(x_span) || !Number.isFinite(y_span)) {
				return null;
			}

			const min_x_span = Math.min(MIN_VIEW_SPAN, range_span_x);
			const min_y_span = Math.min(MIN_VIEW_SPAN, range_span_y);
			x_span = clamp(x_span, min_x_span, range_span_x);
			y_span = clamp(y_span, min_y_span, range_span_y);
			if (x_span > MAX_VIEW_SPAN || y_span > MAX_VIEW_SPAN) {
				return null;
			}

			const center_x_candidate = (bounds.x_min + bounds.x_max) * 0.5;
			const center_y_candidate = (bounds.y_min + bounds.y_max) * 0.5;
			const min_center_x = active_range.x_min + x_span * 0.5;
			const max_center_x = active_range.x_max - x_span * 0.5;
			const min_center_y = active_range.y_min + y_span * 0.5;
			const max_center_y = active_range.y_max - y_span * 0.5;

			const center_x =
				min_center_x > max_center_x
					? (active_range.x_min + active_range.x_max) * 0.5
					: clamp(center_x_candidate, min_center_x, max_center_x);
			const center_y =
				min_center_y > max_center_y
					? (active_range.y_min + active_range.y_max) * 0.5
					: clamp(center_y_candidate, min_center_y, max_center_y);

			return {
				x_min: center_x - x_span * 0.5,
				x_max: center_x + x_span * 0.5,
				y_min: center_y - y_span * 0.5,
				y_max: center_y + y_span * 0.5
			};
		}

		let x_span = bounds.x_max - bounds.x_min;
		let y_span = bounds.y_max - bounds.y_min;
		if (x_span < MIN_VIEW_SPAN) {
			const center = (bounds.x_min + bounds.x_max) * 0.5;
			x_span = MIN_VIEW_SPAN;
			bounds.x_min = center - x_span * 0.5;
			bounds.x_max = center + x_span * 0.5;
		}
		if (y_span < MIN_VIEW_SPAN) {
			const center = (bounds.y_min + bounds.y_max) * 0.5;
			y_span = MIN_VIEW_SPAN;
			bounds.y_min = center - y_span * 0.5;
			bounds.y_max = center + y_span * 0.5;
		}
		if (x_span > MAX_VIEW_SPAN || y_span > MAX_VIEW_SPAN) {
			return null;
		}

		return bounds;
	};

	const same_view_bounds = (
		left: CurveViewBounds | null | undefined,
		right: CurveViewBounds | null | undefined
	): boolean => {
		if (!left || !right) {
			return left === right;
		}
		return (
			Math.abs(left.x_min - right.x_min) <= CURVE_EPSILON &&
			Math.abs(left.x_max - right.x_max) <= CURVE_EPSILON &&
			Math.abs(left.y_min - right.y_min) <= CURVE_EPSILON &&
			Math.abs(left.y_max - right.y_max) <= CURVE_EPSILON
		);
	};

	const cancel_view_bounds_animation = (): void => {
		if (view_bounds_animation_frame !== 0 && typeof window !== 'undefined') {
			window.cancelAnimationFrame(view_bounds_animation_frame);
		}
		view_bounds_animation_frame = 0;
		view_bounds_animation_target = null;
	};

	const set_view_bounds_instantly = (candidate: CurveViewBounds): void => {
		const sanitized = sanitize_view_bounds({ ...candidate });
		if (!sanitized) {
			return;
		}
		cancel_view_bounds_animation();
		fixed_view_bounds = sanitized;
	};

	const animate_view_bounds_to = (candidate: CurveViewBounds): void => {
		const target_bounds = sanitize_view_bounds({ ...candidate });
		if (!target_bounds) {
			return;
		}

		const current_bounds = current_view_bounds();
		if (!current_bounds) {
			set_view_bounds_instantly(target_bounds);
			return;
		}

		const start_bounds = sanitize_view_bounds({ ...current_bounds });
		if (
			!start_bounds ||
			same_view_bounds(start_bounds, target_bounds) ||
			typeof window === 'undefined'
		) {
			set_view_bounds_instantly(target_bounds);
			return;
		}

		cancel_view_bounds_animation();
		fixed_view_bounds = start_bounds;
		view_bounds_animation_target = target_bounds;
		const started_at_ms = typeof performance !== 'undefined' ? performance.now() : Date.now();
		const interpolate = (start: number, end: number, progress: number): number =>
			start + (end - start) * progress;
		const step = (now_ms: number): void => {
			const elapsed_ms = Math.max(0, now_ms - started_at_ms);
			const progress = Math.min(1, elapsed_ms / VIEW_BOUNDS_ANIMATION_DURATION_MS);
			const eased_progress = 1 - Math.pow(1 - progress, 3);
			const interpolated_bounds = sanitize_view_bounds({
				x_min: interpolate(start_bounds.x_min, target_bounds.x_min, eased_progress),
				x_max: interpolate(start_bounds.x_max, target_bounds.x_max, eased_progress),
				y_min: interpolate(start_bounds.y_min, target_bounds.y_min, eased_progress),
				y_max: interpolate(start_bounds.y_max, target_bounds.y_max, eased_progress)
			});
			if (interpolated_bounds) {
				fixed_view_bounds = interpolated_bounds;
			}
			if (progress >= 1) {
				view_bounds_animation_frame = 0;
				view_bounds_animation_target = null;
				fixed_view_bounds = target_bounds;
				return;
			}
			view_bounds_animation_frame = window.requestAnimationFrame(step);
		};
		view_bounds_animation_frame = window.requestAnimationFrame(step);
	};

	const apply_view_bounds = (
		candidate: CurveViewBounds,
		options: { animate?: boolean } = {}
	): void => {
		if (options.animate) {
			animate_view_bounds_to(candidate);
			return;
		}
		set_view_bounds_instantly(candidate);
	};

	$effect(() => {
		if (compiled_curve.keys.length !== 0) {
			return;
		}
		if (fixed_view_bounds !== null) {
			cancel_view_bounds_animation();
			fixed_view_bounds = null;
		}
	});

	$effect(() => {
		active_curve_range_constraint;
		const current_fixed = fixed_view_bounds;
		if (!current_fixed) {
			return;
		}
		const sanitized = sanitize_view_bounds({ ...current_fixed });
		if (!sanitized) {
			cancel_view_bounds_animation();
			fixed_view_bounds = null;
			return;
		}
		if (
			Math.abs(current_fixed.x_min - sanitized.x_min) > CURVE_EPSILON ||
			Math.abs(current_fixed.x_max - sanitized.x_max) > CURVE_EPSILON ||
			Math.abs(current_fixed.y_min - sanitized.y_min) > CURVE_EPSILON ||
			Math.abs(current_fixed.y_max - sanitized.y_max) > CURVE_EPSILON
		) {
			fixed_view_bounds = sanitized;
		}
	});

	$effect(() => {
		compiled_curve;
		active_curve_range_constraint;
		if (fixed_view_bounds !== null) {
			return;
		}
		const next_bounds = default_curve_view_bounds();
		if (next_bounds) {
			fixed_view_bounds = next_bounds;
		}
	});

	const pan_view_by_pixels = (delta_screen_x: number, delta_screen_y: number): void => {
		const base = current_view_bounds();
		if (!base || !interaction_transform) {
			return;
		}
		const units_per_px_x =
			(base.x_max - base.x_min) / Math.max(1, interaction_transform.plot_width);
		const units_per_px_y =
			(base.y_max - base.y_min) / Math.max(1, interaction_transform.plot_height);
		apply_view_bounds({
			x_min: base.x_min - delta_screen_x * units_per_px_x,
			x_max: base.x_max - delta_screen_x * units_per_px_x,
			y_min: base.y_min + delta_screen_y * units_per_px_y,
			y_max: base.y_max + delta_screen_y * units_per_px_y
		});
	};

	const zoom_view_at_screen = (
		screen_x: number,
		screen_y: number,
		zoom_factor_x: number,
		zoom_factor_y: number
	): void => {
		if (!interaction_transform) {
			return;
		}
		const anchor = screen_to_curve_point(screen_x, screen_y);
		const base = current_view_bounds();
		if (!anchor || !base) {
			return;
		}

		const safe_zoom_x = clamp(zoom_factor_x, 0.08, 24);
		const safe_zoom_y = clamp(zoom_factor_y, 0.08, 24);
		const left = anchor.position - base.x_min;
		const right = base.x_max - anchor.position;
		const down = anchor.value - base.y_min;
		const up = base.y_max - anchor.value;

		apply_view_bounds({
			x_min: anchor.position - left * safe_zoom_x,
			x_max: anchor.position + right * safe_zoom_x,
			y_min: anchor.value - down * safe_zoom_y,
			y_max: anchor.value + up * safe_zoom_y
		});
	};

	const normalize_wheel_delta = (event: WheelEvent): { x: number; y: number } => {
		let scale = 1;
		if (event.deltaMode === 1) {
			scale = rem_base_px * 1.3;
		} else if (event.deltaMode === 2) {
			const rect = canvas_element?.getBoundingClientRect();
			const width = rect ? Math.max(1, rect.width) : 1;
			const height = rect ? Math.max(1, rect.height) : 1;
			scale = Math.max(width, height);
		}
		return {
			x: event.deltaX * scale,
			y: event.deltaY * scale
		};
	};

	const nearest_key = (screen_x: number, screen_y: number, threshold_px: number): NodeId | null => {
		if (!interaction_transform) {
			return null;
		}
		let result: NodeId | null = null;
		let best_distance = Number.POSITIVE_INFINITY;
		for (const [key_id, point] of interaction_transform.key_screen_points.entries()) {
			const dx = point.x - screen_x;
			const dy = point.y - screen_y;
			const distance = Math.hypot(dx, dy);
			if (distance > threshold_px || distance >= best_distance) {
				continue;
			}
			best_distance = distance;
			result = key_id;
		}
		return result;
	};

	const nearest_curve_hit = (
		screen_x: number,
		screen_y: number,
		threshold_px: number
	): CurveSegmentHit | null => {
		if (!interaction_transform) {
			return null;
		}
		const curve_x = interaction_transform.curve_screen_x;
		const curve_y = interaction_transform.curve_screen_y;
		const owners = interaction_transform.curve_owner_key_ids;
		if (curve_x.length < 2 || curve_y.length < 2 || owners.length < 1) {
			return null;
		}

		let result: CurveSegmentHit | null = null;
		let best_distance_sq = threshold_px * threshold_px;
		for (let index = 0; index + 1 < curve_x.length; index += 1) {
			const owner = owners[index] ?? null;
			if (owner === null) {
				continue;
			}

			const ax = curve_x[index];
			const ay = curve_y[index];
			const bx = curve_x[index + 1];
			const by = curve_y[index + 1];
			const dx = bx - ax;
			const dy = by - ay;
			const length_sq = dx * dx + dy * dy;
			let t = 0;
			if (length_sq > CURVE_EPSILON) {
				t = clamp(((screen_x - ax) * dx + (screen_y - ay) * dy) / length_sq, 0, 1);
			}
			const closest_x = ax + dx * t;
			const closest_y = ay + dy * t;
			const distance_sq =
				(closest_x - screen_x) * (closest_x - screen_x) +
				(closest_y - screen_y) * (closest_y - screen_y);
			if (distance_sq >= best_distance_sq) {
				continue;
			}
			const curve_point = screen_to_curve_point(closest_x, closest_y);
			if (!curve_point) {
				continue;
			}
			if (!Number.isFinite(curve_point.position) || !Number.isFinite(curve_point.value)) {
				continue;
			}
			best_distance_sq = distance_sq;
			result = {
				owner_key_id: owner,
				curve_position: curve_point.position,
				curve_value: curve_point.value
			};
		}
		return result;
	};

	const nearest_curve_owner_key = (
		screen_x: number,
		screen_y: number,
		threshold_px: number
	): NodeId | null => nearest_curve_hit(screen_x, screen_y, threshold_px)?.owner_key_id ?? null;

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
		const selection = active_box_selection;
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
		if (!interaction_transform) {
			return [];
		}
		const selected = new Set<NodeId>();
		for (const [key_id, point] of interaction_transform.key_screen_points.entries()) {
			if (point_in_screen_rect(point.x, point.y, rect)) {
				selected.add(key_id);
			}
		}
		return ordered_node_ids(selected);
	};

	const collect_curve_owner_key_ids_in_screen_rect = (rect: ScreenRect): NodeId[] => {
		if (!interaction_transform) {
			return [];
		}
		const selected = new Set<NodeId>();
		const curve_x = interaction_transform.curve_screen_x;
		const curve_y = interaction_transform.curve_screen_y;
		const owners = interaction_transform.curve_owner_key_ids;
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

	const finish_box_selection = (pointer_id?: number): void => {
		const active = active_box_selection;
		if (!active) {
			return;
		}
		if (pointer_id !== undefined && active.pointer_id !== pointer_id) {
			return;
		}

		const release_pointer_id = pointer_id ?? active.pointer_id;
		if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
			canvas_element.releasePointerCapture(release_pointer_id);
		}

		const rect = box_selection_rect(active);
		const drag_size = Math.max(rect.right - rect.left, rect.bottom - rect.top);
		const drag_threshold = Math.max(3, rem_base_px * 0.24);
		const performed_drag = drag_size >= drag_threshold;
		if (!performed_drag) {
			if (active.mode === 'replace') {
				set_single_selected_key(null);
				set_single_selected_curve_owner_key(null);
			}
			active_box_selection = null;
			return;
		}

		const key_hits = collect_key_ids_in_screen_rect(rect);
		const curve_hits = collect_curve_owner_key_ids_in_screen_rect(rect);
		const current_key_selection =
			selected_key_ids.length > 0
				? selected_key_ids
				: selected_key_id === null
					? []
					: [selected_key_id];
		if (key_hits.length > 0) {
			const next_key_selection = apply_selection_mode(current_key_selection, key_hits, active.mode);
			selected_key_ids = next_key_selection;
			selected_key_id = next_key_selection[0] ?? null;
			clear_selected_curve_owner_keys();
			active_box_selection = null;
			return;
		}
		if (active.mode === 'replace' || curve_hits.length > 0) {
			clear_selected_keys();
		}
		selected_curve_owner_key_ids = apply_selection_mode(
			selected_curve_owner_key_ids,
			curve_hits,
			active.mode
		);
		active_box_selection = null;
	};

	const nearest_bezier_handle = (
		screen_x: number,
		screen_y: number,
		threshold_px: number
	): BezierHandleRef | null => {
		if (!interaction_transform || bezier_handle_controls.length === 0) {
			return null;
		}

		const transform = interaction_transform;
		const x_span = Math.max(CURVE_EPSILON, transform.x_max - transform.x_min);
		const y_span = Math.max(CURVE_EPSILON, transform.y_max - transform.y_min);
		const to_screen_x = (position: number): number =>
			transform.plot_left + ((position - transform.x_min) / x_span) * transform.plot_width;
		const to_screen_y = (value: number): number =>
			transform.plot_top +
			transform.plot_height -
			((value - transform.y_min) / y_span) * transform.plot_height;

		let nearest: BezierHandleRef | null = null;
		let best_distance = Number.POSITIVE_INFINITY;
		const consider = (ref: BezierHandleRef, position: number, value: number): void => {
			const x = to_screen_x(position);
			const y = to_screen_y(value);
			const distance = Math.hypot(x - screen_x, y - screen_y);
			if (distance > threshold_px || distance >= best_distance) {
				return;
			}
			best_distance = distance;
			nearest = ref;
		};

		for (const control of bezier_handle_controls) {
			consider(control.ref, control.handle_position, control.handle_value);
		}
		return nearest;
	};

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

		const out_control = all_bezier_handle_control_by_id.get(
			bezier_handle_control_id({ key_id, kind: 'out' })
		);
		const in_control = all_bezier_handle_control_by_id.get(
			bezier_handle_control_id({ key_id, kind: 'in' })
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
		return Math.abs(cross) / denominator <= HANDLE_ALIGNMENT_COSINE_TOLERANCE;
	};

	const resolve_curve_drag_opposite_alignment = (
		anchor_key_id: NodeId,
		primary_ref: BezierHandleRef
	): CurveOppositeAlignment | null => {
		const primary = all_bezier_handle_control_by_id.get(bezier_handle_control_id(primary_ref));
		if (!primary) {
			return null;
		}
		const opposite_candidates = all_bezier_handle_refs_by_anchor_key_id.get(anchor_key_id) ?? [];
		let opposite_ref: BezierHandleRef | null = null;
		for (const candidate of opposite_candidates) {
			if (bezier_handle_control_id(candidate) === bezier_handle_control_id(primary_ref)) {
				continue;
			}
			opposite_ref = candidate;
			break;
		}
		if (!opposite_ref) {
			return null;
		}
		const opposite = all_bezier_handle_control_by_id.get(bezier_handle_control_id(opposite_ref));
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
		const opposite_control = all_bezier_handle_control_by_id.get(
			bezier_handle_control_id(alignment.opposite_ref)
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
	): { position: number; value: number } => {
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
	): { position: number; value: number } => ({
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
		curve_point: { position: number; value: number },
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
			const full_strength_px = Math.max(1, rem_base_px * CURVE_SCULPT_SHIFT_FULL_STRENGTH_PX);
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
			): { x: number; y: number } => ({
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

		const curve_point_for_key = (key_id: NodeId): { position: number; value: number } | null => {
			const translated = commit_targets.get(key_id);
			if (translated) {
				return translated;
			}
			const key = parsed_key_by_id.get(key_id);
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

	const flush_queued_drag = (): void => {
		if (!queued_drag_targets || queued_drag_targets.size === 0) {
			return;
		}
		for (const [key_id, target] of queued_drag_targets.entries()) {
			const key = parsed_key_by_id.get(key_id);
			if (!key) {
				continue;
			}
			void set_numeric_param(key.position_param, target.position);
			void set_numeric_param(key.value_param, target.value);
		}
		queued_drag_targets = null;
	};

	const bezier_param_refs_by_key_id = (key_id: NodeId): Record<string, ParamNodeRef> | null => {
		const nodes_by_id = graphNodesById;
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
		const refs = all_bezier_handle_refs_by_anchor_key_id.get(anchor_key_id) ?? [];
		for (const ref of refs) {
			if (ref.kind !== kind) {
				continue;
			}
			const control = all_bezier_handle_control_by_id.get(bezier_handle_control_id(ref));
			if (control) {
				return control;
			}
		}
		return null;
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
			slope_from_bezier_control(bezier_control_for_anchor_kind(start.node_id, 'out')) ??
			left_secant_slope;
		const end_slope =
			slope_from_bezier_control(bezier_control_for_anchor_kind(end.node_id, 'in')) ??
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
		existing_easing_updates: Array<{ key_id: NodeId; values: Record<string, ParamValue> }>;
	} => {
		if (parsed_keys.length === 0) {
			return {
				new_key_easing_values: null,
				existing_easing_updates: []
			};
		}
		const first_key = parsed_keys[0];
		if (position < first_key.position - CURVE_EPSILON) {
			const secant_slope =
				(first_key.value - value) / Math.max(CURVE_EPSILON, first_key.position - position);
			const end_slope =
				slope_from_bezier_control(bezier_control_for_anchor_kind(first_key.node_id, 'out')) ??
				secant_slope;
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
		const last_key = parsed_keys[parsed_keys.length - 1];
		if (position > last_key.position + CURVE_EPSILON) {
			const secant_slope =
				(value - last_key.value) / Math.max(CURVE_EPSILON, position - last_key.position);
			const start_slope =
				slope_from_bezier_control(bezier_control_for_anchor_kind(last_key.node_id, 'in')) ??
				secant_slope;
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

	const flush_queued_bezier_handle = (): void => {
		if (!queued_bezier_handle_targets || queued_bezier_handle_targets.size === 0) {
			return;
		}
		const targets = queued_bezier_handle_targets;
		queued_bezier_handle_targets = null;

		const send_if_changed = (ref: ParamNodeRef | undefined, value: number): void => {
			if (!ref || !Number.isFinite(value)) {
				return;
			}
			const current = param_number_value(ref.value, value);
			if (Math.abs(current - value) <= 1e-9) {
				return;
			}
			void set_numeric_param(ref, value);
		};

		for (const [key_id, preview] of targets.entries()) {
			const easing_params = bezier_param_refs_by_key_id(key_id);
			if (!easing_params) {
				continue;
			}
			send_if_changed(easing_params[DECL_OUT_POSITION], preview.out_position);
			send_if_changed(easing_params[DECL_OUT_VALUE], preview.out_value);
			send_if_changed(easing_params[DECL_IN_POSITION], preview.in_position);
			send_if_changed(easing_params[DECL_IN_VALUE], preview.in_value);
		}
	};

	const ensure_drag_commit_frame = (): void => {
		if (drag_commit_raf_id !== 0 || typeof window === 'undefined') {
			return;
		}
		drag_commit_raf_id = window.requestAnimationFrame(() => {
			drag_commit_raf_id = 0;
			flush_queued_drag();
			flush_queued_bezier_handle();
		});
	};

	const queue_drag_commit = (targets: Map<NodeId, DragPreview>): void => {
		queued_drag_targets = targets;
		ensure_drag_commit_frame();
	};

	const queue_bezier_handle_commit = (targets: Map<NodeId, BezierEasingPreview>): void => {
		queued_bezier_handle_targets = targets;
		ensure_drag_commit_frame();
	};

	const begin_drag_edit_session = (label: string, prefix: string): void => {
		const edit_session = createUiEditSession(label, prefix);
		drag_edit_session = edit_session;
		drag_edit_session_begin_promise = edit_session.begin();
	};

	const finish_drag = (pointer_id?: number): void => {
		let did_finish = false;
		const drag_previews_to_clear = new Map<NodeId, DragPreview>();
		const easing_previews_to_clear = new Map<NodeId, BezierEasingPreview>();
		if (active_drag && (pointer_id === undefined || active_drag.pointer_id === pointer_id)) {
			flush_queued_drag();
			flush_queued_bezier_handle();
			for (const entry of active_drag.origins) {
				const preview = drag_preview_by_key_id.get(entry.key_id);
				if (preview) {
					drag_previews_to_clear.set(entry.key_id, preview);
				}
			}
			for (const key_id of active_drag.touched_bezier_key_ids) {
				const preview = easing_drag_preview_by_key_id.get(key_id);
				if (preview) {
					easing_previews_to_clear.set(key_id, preview);
				}
			}
			const release_pointer_id = pointer_id ?? active_drag.pointer_id;
			if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
				canvas_element.releasePointerCapture(release_pointer_id);
			}
			active_drag = null;
			did_finish = true;
		}
		if (
			active_bezier_handle_drag &&
			(pointer_id === undefined || active_bezier_handle_drag.pointer_id === pointer_id)
		) {
			flush_queued_bezier_handle();
			for (const key_id of active_bezier_handle_drag.touched_key_ids) {
				const preview = easing_drag_preview_by_key_id.get(key_id);
				if (preview) {
					easing_previews_to_clear.set(key_id, preview);
				}
			}
			hover_bezier_handle = null;
			const release_pointer_id = pointer_id ?? active_bezier_handle_drag.pointer_id;
			if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
				canvas_element.releasePointerCapture(release_pointer_id);
			}
			active_bezier_handle_drag = null;
			did_finish = true;
		}
		if (
			active_curve_drag &&
			(pointer_id === undefined || active_curve_drag.pointer_id === pointer_id)
		) {
			flush_queued_bezier_handle();
			for (const key_id of active_curve_drag.touched_key_ids) {
				const preview = easing_drag_preview_by_key_id.get(key_id);
				if (preview) {
					easing_previews_to_clear.set(key_id, preview);
				}
			}
			hover_bezier_handle = null;
			const release_pointer_id = pointer_id ?? active_curve_drag.pointer_id;
			if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
				canvas_element.releasePointerCapture(release_pointer_id);
			}
			active_curve_drag = null;
			did_finish = true;
		}
		if (did_finish && drag_edit_session) {
			const edit_session = drag_edit_session;
			const begin_promise = drag_edit_session_begin_promise;
			drag_edit_session = null;
			drag_edit_session_begin_promise = null;
			void (async () => {
				if (begin_promise) {
					await begin_promise;
				}
				if (pending_param_write_promises.size > 0) {
					await Promise.allSettled([...pending_param_write_promises]);
				}
				clear_drag_previews_if_unchanged(drag_previews_to_clear);
				clear_easing_drag_previews_if_unchanged(easing_previews_to_clear);
				await edit_session.end();
			})();
		} else if (did_finish) {
			clear_drag_previews_if_unchanged(drag_previews_to_clear);
			clear_easing_drag_previews_if_unchanged(easing_previews_to_clear);
		}
	};

	const finish_canvas_pan = (pointer_id?: number): void => {
		if (!active_canvas_pan) {
			return;
		}
		if (pointer_id !== undefined && active_canvas_pan.pointer_id !== pointer_id) {
			return;
		}
		const release_pointer_id = pointer_id ?? active_canvas_pan.pointer_id;
		if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
			canvas_element.releasePointerCapture(release_pointer_id);
		}
		active_canvas_pan = null;
	};

	const finish_curve_draw = (pointer_id?: number, commit = true): boolean => {
		if (!active_curve_draw) {
			return false;
		}
		if (pointer_id !== undefined && active_curve_draw.pointer_id !== pointer_id) {
			return false;
		}
		const release_pointer_id = pointer_id ?? active_curve_draw.pointer_id;
		if (canvas_element && canvas_element.hasPointerCapture(release_pointer_id)) {
			canvas_element.releasePointerCapture(release_pointer_id);
		}
		const points = active_curve_draw.points;
		active_curve_draw = null;
		if (commit) {
			void commit_curve_draw(points);
		}
		return true;
	};

	const update_bezier_handle_drag_preview = (
		drag: ActiveBezierHandleDrag,
		curve_point: { position: number; value: number },
		individual_mode: boolean
	): Map<NodeId, BezierEasingPreview> => {
		const targets = new Map<NodeId, BezierEasingPreview>();
		const control = all_bezier_handle_control_by_id.get(bezier_handle_control_id(drag.ref));
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

		const opposite = all_bezier_handle_control_by_id.get(
			bezier_handle_control_id(drag.opposite_ref)
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

	const trigger_canvas_key_add = (
		curve_point: { position: number; value: number },
		curve_hit: CurveSegmentHit | null
	): void => {
		void add_key_at(curve_point.position, curve_point.value, {
			preserve_curve_shape: curve_hit !== null,
			preferred_owner_key_id: curve_hit?.owner_key_id ?? null
		});
		hover_key_id = null;
		hover_curve_owner_key_id = curve_hit?.owner_key_id ?? null;
		hover_bezier_handle = null;
	};

	const handle_canvas_double_activation = (
		key_id: NodeId | null,
		bezier_handle: BezierHandleRef | null,
		curve_hit: CurveSegmentHit | null,
		curve_point: { position: number; value: number } | null
	): boolean => {
		if (key_id !== null) {
			void remove_key_ids([key_id]);
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			return true;
		}
		if (bezier_handle || !curve_point) {
			return false;
		}
		trigger_canvas_key_add(curve_point, curve_hit);
		return true;
	};

	const handle_canvas_shortcut_click = (
		key_id: NodeId | null,
		curve_hit: CurveSegmentHit | null,
		curve_point: { position: number; value: number } | null,
		shift_key: boolean
	): boolean => {
		if (key_id !== null) {
			void remove_key_ids([key_id]);
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			return true;
		}
		if (curve_hit) {
			set_single_selected_curve_owner_key(curve_hit.owner_key_id);
			hover_key_id = null;
			hover_curve_owner_key_id = curve_hit.owner_key_id;
			hover_bezier_handle = null;
			void cycle_easing_kind(curve_hit.owner_key_id, shift_key ? -1 : 1);
			return true;
		}
		if (!shift_key && curve_point) {
			trigger_canvas_key_add(curve_point, null);
			return true;
		}
		return false;
	};

	const on_canvas_context_menu = (event: MouseEvent): void => {
		if (!interaction_transform || !canvas_element) {
			return;
		}
		const point = canvas_local_point(event);
		if (!point) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		blur_active_editable_element();
		canvas_element.focus();
		is_canvas_focused = true;
		hover_curve_position = screen_to_curve_point(point.x, point.y);

		const key_id = nearest_key(point.x, point.y, key_hit_threshold_px());
		if (key_id !== null) {
			set_single_selected_key(key_id);
			hover_key_id = key_id;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			open_curve_context_menu({ kind: 'key', key_id }, event.clientX, event.clientY);
			return;
		}

		const curve_hit = nearest_curve_hit(point.x, point.y, curve_hit_threshold_px());
		if (curve_hit) {
			set_single_selected_curve_owner_key(curve_hit.owner_key_id);
			hover_key_id = null;
			hover_curve_owner_key_id = curve_hit.owner_key_id;
			hover_bezier_handle = null;
			open_curve_context_menu(
				{ kind: 'curve', key_id: curve_hit.owner_key_id },
				event.clientX,
				event.clientY
			);
			return;
		}

		hover_key_id = null;
		hover_curve_owner_key_id = null;
		hover_bezier_handle = null;
		open_curve_context_menu({ kind: 'all' }, event.clientX, event.clientY);
	};

	const on_canvas_pointer_down = (event: PointerEvent): void => {
		if (!interaction_transform || !canvas_element) {
			return;
		}
		const point = canvas_local_point(event);
		if (!point) {
			return;
		}
		blur_active_editable_element();
		canvas_element.focus();
		is_canvas_focused = true;
		hover_curve_position = screen_to_curve_point(point.x, point.y);

		if (event.button === 1) {
			const start_bounds = current_view_bounds();
			if (!start_bounds) {
				return;
			}
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			active_canvas_pan = {
				pointer_id: event.pointerId,
				start_screen_x: point.x,
				start_screen_y: point.y,
				start_bounds
			};
			apply_view_bounds(start_bounds);
			canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}
		if (event.button !== 0) {
			return;
		}

		const curve_point = hover_curve_position;
		const selection = nearest_key(point.x, point.y, key_hit_threshold_px());
		const bezier_handle = nearest_bezier_handle(point.x, point.y, bezier_handle_hit_threshold_px());
		const curve_hit = nearest_curve_hit(point.x, point.y, curve_hit_threshold_px());
		const is_double_activation = matches_canvas_double_activation(
			last_canvas_primary_down,
			event,
			point.x,
			point.y
		);
		last_canvas_primary_down = {
			time_stamp: event.timeStamp,
			screen_x: point.x,
			screen_y: point.y
		};

		if (is_double_activation) {
			if (handle_canvas_double_activation(selection, bezier_handle, curve_hit, curve_point)) {
				last_canvas_primary_down = null;
				event.preventDefault();
				return;
			}
		}

		if (
			shortcut_modifier_active(event) &&
			handle_canvas_shortcut_click(selection, curve_hit, curve_point, event.shiftKey)
		) {
			event.preventDefault();
			return;
		}

		if (event.ctrlKey && event.shiftKey && selection === null && !bezier_handle && !curve_hit) {
			if (!curve_point) {
				return;
			}
			active_curve_draw = record_curve_draw_point(
				{
					pointer_id: event.pointerId,
					points: [],
					path_buckets: []
				},
				point.x,
				curve_point
			);
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			active_box_selection = null;
			canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}

		const additive_selection = event.shiftKey;
		if (selection !== null) {
			if (additive_selection) {
				add_selected_key(selection);
				selected_key_id = selection;
				hover_curve_owner_key_id = null;
				event.preventDefault();
				return;
			}

			if (!has_selected_key(selection)) {
				set_single_selected_key(selection);
			} else {
				selected_key_id = selection;
			}

			const drag_key_ids = has_selected_key(selection) ? selected_key_ids : [selection];
			const drag_origins: Array<{ key_id: NodeId; position: number; value: number }> = [];
			for (const key_id of drag_key_ids) {
				const key = parsed_key_by_id.get(key_id);
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
			const position_delta_constraint = constrainKeysToNeighbors
				? build_key_drag_position_delta_constraint(drag_origins)
				: null;
			const bezier_alignment_constraints =
				build_key_drag_bezier_alignment_constraints(drag_key_ids);

			begin_drag_edit_session('Move Curve Key', 'curve-key-drag');
			active_drag = {
				pointer_id: event.pointerId,
				anchor_key_id: anchor_origin.key_id,
				anchor_start_position: anchor_origin.position,
				anchor_start_value: anchor_origin.value,
				origins: drag_origins,
				position_delta_constraint,
				bezier_alignment_constraints,
				touched_bezier_key_ids: new Set<NodeId>()
			};
			canvas_element.setPointerCapture(event.pointerId);
			hover_curve_owner_key_id = null;
			event.preventDefault();
			return;
		}

		if (bezier_handle) {
			const control = all_bezier_handle_control_by_id.get(bezier_handle_control_id(bezier_handle));
			if (!control) {
				return;
			}
			hover_bezier_handle = bezier_handle;
			hover_key_id = control.anchor_key_id;
			hover_curve_owner_key_id = null;

			const opposite_candidates =
				all_bezier_handle_refs_by_anchor_key_id.get(control.anchor_key_id) ?? [];
			let opposite_ref: BezierHandleRef | null = null;
			for (const candidate of opposite_candidates) {
				if (bezier_handle_control_id(candidate) === bezier_handle_control_id(bezier_handle)) {
					continue;
				}
				opposite_ref = candidate;
				break;
			}

			let opposite_length = 0;
			if (opposite_ref) {
				const opposite_control = all_bezier_handle_control_by_id.get(
					bezier_handle_control_id(opposite_ref)
				);
				if (opposite_control) {
					const opposite_dx = opposite_control.handle_position - opposite_control.anchor_position;
					const opposite_dy = opposite_control.handle_value - opposite_control.anchor_value;
					opposite_length = Math.hypot(opposite_dx, opposite_dy);
				}
			}

			begin_drag_edit_session('Adjust Curve Bezier Handle', 'curve-bezier-drag');
			active_bezier_handle_drag = {
				pointer_id: event.pointerId,
				ref: bezier_handle,
				opposite_ref,
				opposite_length,
				touched_key_ids: new Set<NodeId>([bezier_handle.key_id])
			};
			canvas_element.setPointerCapture(event.pointerId);
			event.preventDefault();
			return;
		}

		if (curve_hit) {
			const curve_selection = curve_hit.owner_key_id;
			if (additive_selection) {
				add_selected_curve_owner_key(curve_selection);
			} else if (
				!has_selected_curve_owner_key(curve_selection) ||
				selected_curve_owner_key_ids.length !== 1
			) {
				set_single_selected_curve_owner_key(curve_selection);
			}

			const segment = bezier_segment_by_owner_key_id.get(curve_selection);
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
			const start_opposite_alignment = resolve_curve_drag_opposite_alignment(segment.start_key_id, {
				key_id: segment.key_id,
				kind: 'out'
			});
			const end_opposite_alignment = resolve_curve_drag_opposite_alignment(segment.end_key_id, {
				key_id: segment.key_id,
				kind: 'in'
			});

			begin_drag_edit_session('Sculpt Curve Segment', 'curve-segment-drag');
			active_curve_drag = {
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
			canvas_element.setPointerCapture(event.pointerId);
			hover_curve_owner_key_id = curve_selection;
			hover_key_id = null;
			event.preventDefault();
			return;
		}

		const mode: SelectionMode = additive_selection ? 'add' : 'replace';
		active_box_selection = {
			pointer_id: event.pointerId,
			start_screen_x: point.x,
			start_screen_y: point.y,
			current_screen_x: point.x,
			current_screen_y: point.y,
			mode
		};
		hover_key_id = null;
		hover_curve_owner_key_id = null;
		hover_bezier_handle = null;
		canvas_element.setPointerCapture(event.pointerId);
		event.preventDefault();
	};

	const on_canvas_pointer_move = (event: PointerEvent): void => {
		const point = canvas_local_point(event);
		if (!point) {
			return;
		}
		hover_curve_position = screen_to_curve_point(point.x, point.y);

		if (active_curve_draw && active_curve_draw.pointer_id === event.pointerId) {
			const curve_point = hover_curve_position;
			if (!curve_point) {
				return;
			}
			active_curve_draw = record_curve_draw_point(active_curve_draw, point.x, curve_point);
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			return;
		}

		if (
			active_canvas_pan &&
			active_canvas_pan.pointer_id === event.pointerId &&
			interaction_transform
		) {
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			const delta_x = point.x - active_canvas_pan.start_screen_x;
			const delta_y = point.y - active_canvas_pan.start_screen_y;
			const start = active_canvas_pan.start_bounds;
			const units_per_px_x =
				(start.x_max - start.x_min) / Math.max(1, interaction_transform.plot_width);
			const units_per_px_y =
				(start.y_max - start.y_min) / Math.max(1, interaction_transform.plot_height);
			apply_view_bounds({
				x_min: start.x_min - delta_x * units_per_px_x,
				x_max: start.x_max - delta_x * units_per_px_x,
				y_min: start.y_min + delta_y * units_per_px_y,
				y_max: start.y_max + delta_y * units_per_px_y
			});
			return;
		}

		if (active_box_selection && active_box_selection.pointer_id === event.pointerId) {
			active_box_selection = {
				...active_box_selection,
				current_screen_x: point.x,
				current_screen_y: point.y
			};
			hover_key_id = null;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			return;
		}

		if (active_bezier_handle_drag && active_bezier_handle_drag.pointer_id === event.pointerId) {
			const curve_point = screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			const preview_updates = update_bezier_handle_drag_preview(
				active_bezier_handle_drag,
				curve_point,
				event.altKey
			);
			if (preview_updates.size === 0) {
				return;
			}
			for (const [key_id, preview] of preview_updates.entries()) {
				set_easing_drag_preview(key_id, preview);
				active_bezier_handle_drag.touched_key_ids.add(key_id);
			}
			queue_bezier_handle_commit(preview_updates);
			hover_bezier_handle = active_bezier_handle_drag.ref;
			const active_control = all_bezier_handle_control_by_id.get(
				bezier_handle_control_id(active_bezier_handle_drag.ref)
			);
			if (active_control) {
				hover_key_id = active_control.anchor_key_id;
			}
			hover_curve_owner_key_id = null;
			return;
		}

		if (active_curve_drag && active_curve_drag.pointer_id === event.pointerId) {
			const curve_point = screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			hover_bezier_handle = null;
			const preview_updates = update_curve_drag_preview(
				active_curve_drag,
				curve_point,
				point.x,
				event.altKey,
				event.shiftKey
			);
			if (preview_updates.size === 0) {
				return;
			}
			const updated_key_ids = new Set<NodeId>(preview_updates.keys());
			for (const key_id of [...active_curve_drag.touched_key_ids]) {
				if (updated_key_ids.has(key_id)) {
					continue;
				}
				clear_easing_drag_preview(key_id);
				active_curve_drag.touched_key_ids.delete(key_id);
			}
			for (const [key_id, preview] of preview_updates.entries()) {
				set_easing_drag_preview(key_id, preview);
				active_curve_drag.touched_key_ids.add(key_id);
			}
			queue_bezier_handle_commit(preview_updates);
			hover_key_id = null;
			hover_curve_owner_key_id = active_curve_drag.owner_key_id;
			return;
		}

		if (active_drag && active_drag.pointer_id === event.pointerId) {
			const curve_point = screen_to_curve_point(point.x, point.y);
			if (!curve_point) {
				return;
			}
			hover_bezier_handle = null;
			const raw_delta_position = curve_point.position - active_drag.anchor_start_position;
			const delta_position = clamp_key_drag_position_delta(active_drag, raw_delta_position);
			const delta_value = curve_point.value - active_drag.anchor_start_value;
			const commit_targets = new Map<NodeId, DragPreview>();
			for (const origin of active_drag.origins) {
				const translated = clamp_curve_point_to_active_range(
					origin.position + delta_position,
					origin.value + delta_value
				);
				set_drag_preview(origin.key_id, translated.position, translated.value);
				commit_targets.set(origin.key_id, translated);
			}
			queue_drag_commit(commit_targets);
			const bezier_alignment_updates = update_key_drag_bezier_alignment_targets(
				active_drag,
				commit_targets
			);
			const updated_key_ids = new Set<NodeId>(bezier_alignment_updates.keys());
			for (const key_id of [...active_drag.touched_bezier_key_ids]) {
				if (updated_key_ids.has(key_id)) {
					continue;
				}
				clear_easing_drag_preview(key_id);
				active_drag.touched_bezier_key_ids.delete(key_id);
			}
			if (bezier_alignment_updates.size > 0) {
				for (const [key_id, preview] of bezier_alignment_updates.entries()) {
					set_easing_drag_preview(key_id, preview);
					active_drag.touched_bezier_key_ids.add(key_id);
				}
				queue_bezier_handle_commit(bezier_alignment_updates);
			}
			hover_curve_owner_key_id = null;
			return;
		}

		const hovered_key = nearest_key(point.x, point.y, key_hit_threshold_px());
		if (hovered_key !== null) {
			hover_key_id = hovered_key;
			hover_curve_owner_key_id = null;
			hover_bezier_handle = null;
			return;
		}

		const hovered_bezier_handle = nearest_bezier_handle(
			point.x,
			point.y,
			bezier_handle_hit_threshold_px()
		);
		hover_bezier_handle = hovered_bezier_handle;
		if (hovered_bezier_handle) {
			const hovered_control = all_bezier_handle_control_by_id.get(
				bezier_handle_control_id(hovered_bezier_handle)
			);
			hover_key_id = hovered_control?.anchor_key_id ?? hovered_bezier_handle.key_id;
			hover_curve_owner_key_id = null;
			return;
		}
		hover_key_id = null;
		hover_curve_owner_key_id = nearest_curve_owner_key(point.x, point.y, curve_hit_threshold_px());
	};

	const on_canvas_pointer_up = (event: PointerEvent): void => {
		if (finish_curve_draw(event.pointerId, true)) {
			return;
		}
		finish_drag(event.pointerId);
		finish_canvas_pan(event.pointerId);
		finish_box_selection(event.pointerId);
	};
	const on_canvas_pointer_cancel = (event: PointerEvent): void => {
		if (finish_curve_draw(event.pointerId, false)) {
			return;
		}
		finish_drag(event.pointerId);
		finish_canvas_pan(event.pointerId);
		finish_box_selection(event.pointerId);
	};
	const on_canvas_pointer_leave = (): void => {
		hover_key_id = null;
		hover_curve_owner_key_id = null;
		hover_bezier_handle = null;
		hover_curve_position = null;
		last_canvas_primary_down = null;
	};

	const on_canvas_focus = (): void => {
		is_canvas_focused = true;
	};

	const on_canvas_blur = (): void => {
		is_canvas_focused = false;
		last_canvas_primary_down = null;
	};

	const on_canvas_wheel = (event: WheelEvent): void => {
		if (!interaction_transform) {
			return;
		}
		const point = canvas_local_point(event);
		if (!point) {
			return;
		}

		const delta = normalize_wheel_delta(event);
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
			pan_view_by_pixels(pan_x, pan_y);
			event.preventDefault();
			return;
		}

		const zoom_sensitivity = event.ctrlKey || event.metaKey ? 0.00135 : 0.0023;
		const zoom_factor = Math.exp(delta.y * zoom_sensitivity);
		zoom_view_at_screen(point.x, point.y, zoom_factor, zoom_factor);
		event.preventDefault();
	};

	const add_key_at = async (
		position: number,
		value: number,
		options: AddKeyOptions = {}
	): Promise<void> => {
		if (!key_creatable_item || adding_key) {
			return;
		}
		const clamped = clamp_curve_point_to_active_range(position, value);
		let target_value = clamped.value;
		let split_easing: PendingSplitEasing | null = null;
		let easing_values = options.easing_values ?? null;
		let existing_easing_updates = options.existing_easing_updates ?? [];
		if (options.preserve_curve_shape) {
			const split_source = find_split_source_segment(
				clamped.position,
				options.preferred_owner_key_id ?? null
			);
			if (split_source) {
				const sampled_value = sample_compiled_curve(compiled_curve, clamped.position);
				if (sampled_value !== null && Number.isFinite(sampled_value)) {
					target_value = clamp_curve_value_to_active_range(sampled_value);
				}
				split_easing = build_split_easing(
					split_source.start,
					split_source.end,
					clamped.position,
					target_value
				);
			}
		} else {
			const split_source = find_split_source_segment(
				clamped.position,
				options.preferred_owner_key_id ?? null
			);
			if (split_source) {
				split_easing = build_middle_insert_easing(
					split_source.start,
					split_source.end,
					clamped.position,
					target_value
				);
			} else {
				const extension = build_endpoint_extension_easing(clamped.position, target_value);
				if (extension.new_key_easing_values) {
					easing_values = extension.new_key_easing_values;
				}
				if (extension.existing_easing_updates.length > 0) {
					existing_easing_updates = extension.existing_easing_updates;
				}
			}
		}
		const edit_session = createUiEditSession('Add Curve Key', 'curve-key-add');
		await edit_session.begin();
		adding_key = true;
		pending_create_target = {
			known_key_ids: new Set(parsed_keys.map((entry) => entry.node_id)),
			created_key_id: null,
			position: clamped.position,
			value: target_value,
			split_easing,
			easing_values,
			existing_easing_updates,
			edit_session
		};
		const created = await sendCreateUserItemIntent(liveNode.node_id, key_creatable_item);
		adding_key = false;
		if (!created.success) {
			pending_create_target = null;
			await edit_session.end();
		}
	};

	const clone_param_value = (value: ParamValue): ParamValue => {
		return JSON.parse(JSON.stringify(value)) as ParamValue;
	};

	const selected_key_ids_ordered = (): NodeId[] => {
		if (selected_key_ids.length > 0) {
			const selected_set = new Set(selected_key_ids);
			return parsed_keys
				.filter((entry) => selected_set.has(entry.node_id))
				.map((entry) => entry.node_id);
		}
		return selected_key_id === null ? [] : [selected_key_id];
	};

	const wait_pending_key_create = async (): Promise<void> => {
		const deadline = Date.now() + 550;
		while (pending_create_target !== null && Date.now() <= deadline) {
			await new Promise((resolve) => {
				setTimeout(resolve, 16);
			});
		}
	};

	const remove_key_ids = async (key_ids: readonly NodeId[]): Promise<boolean> => {
		const ids = ordered_node_ids(key_ids);
		if (ids.length === 0) {
			return false;
		}
		const id_set = new Set(ids);
		const removed = await sendRemoveNodesIntent(ids);
		if (!removed) {
			return false;
		}
		selected_curve_owner_key_ids = selected_curve_owner_key_ids.filter(
			(key_id) => !id_set.has(key_id)
		);
		const remaining_selected_keys = selected_key_ids.filter((key_id) => !id_set.has(key_id));
		if (remaining_selected_keys.length > 0) {
			selected_key_ids = remaining_selected_keys;
			if (selected_key_id === null || id_set.has(selected_key_id)) {
				selected_key_id = remaining_selected_keys[0] ?? null;
			}
			return true;
		}
		if (selected_key_id !== null && !id_set.has(selected_key_id)) {
			selected_key_ids = [selected_key_id];
			return true;
		}
		const fallback = parsed_keys.find((entry) => !id_set.has(entry.node_id))?.node_id ?? null;
		set_single_selected_key(fallback);
		return true;
	};

	const remove_selected_keys = async (): Promise<boolean> => {
		return remove_key_ids(selected_key_ids_ordered());
	};

	const set_all_keys_selected = (): void => {
		const all_ids = parsed_keys.map((entry) => entry.node_id);
		selected_key_ids = all_ids;
		selected_key_id = all_ids[0] ?? null;
		selected_curve_owner_key_ids = [];
	};

	const frame_selected_view = (): boolean => {
		const frame_keys = selected_frame_key_ids()
			.map((key_id) => parsed_key_by_id.get(key_id))
			.filter((key): key is ParsedCurveKey => key !== undefined);
		if (frame_keys.length === 0) {
			home_view();
			return true;
		}

		let x_min = frame_keys[0].position;
		let x_max = frame_keys[frame_keys.length - 1].position;
		const focus_x_span = estimate_frame_x_span(frame_keys);
		if (x_max - x_min <= CURVE_EPSILON) {
			const center = (x_min + x_max) * 0.5;
			x_min = center - focus_x_span * 0.5;
			x_max = center + focus_x_span * 0.5;
		}

		const sampled_y = sampled_curve_value_bounds(x_min, x_max);
		let y_min = Number.POSITIVE_INFINITY;
		let y_max = Number.NEGATIVE_INFINITY;
		for (const key of frame_keys) {
			const value = clamp_curve_value_to_active_range(key.value);
			if (value < y_min) {
				y_min = value;
			}
			if (value > y_max) {
				y_max = value;
			}
		}
		if (sampled_y) {
			y_min = Math.min(y_min, sampled_y.min);
			y_max = Math.max(y_max, sampled_y.max);
		}
		if (!Number.isFinite(y_min) || !Number.isFinite(y_max)) {
			return false;
		}

		const y_center = (y_min + y_max) * 0.5;
		const y_minimum_span =
			y_max - y_min > CURVE_EPSILON ? y_max - y_min : Math.max(0.5, Math.abs(y_center) * 0.2);
		const padded_x = padded_axis_bounds(x_min, x_max, focus_x_span, 0.08);
		const padded_y = padded_axis_bounds(y_min, y_max, y_minimum_span, 0.1);
		if (!padded_x || !padded_y) {
			return false;
		}

		apply_view_bounds(
			{
				x_min: padded_x.min,
				x_max: padded_x.max,
				y_min: padded_y.min,
				y_max: padded_y.max
			},
			{ animate: true }
		);
		return true;
	};

	const home_view = (): void => {
		const next_bounds = default_curve_view_bounds();
		if (!next_bounds) {
			return;
		}
		apply_view_bounds(next_bounds, { animate: true });
	};

	const key_anchor_point = (): { position: number; value: number } => {
		return hover_curve_position ?? axis_midpoint();
	};

	const easing_values_for_key = (key: ParsedCurveKey): Record<string, ParamValue> => {
		const values: Record<string, ParamValue> = {};
		if (!key.easing) {
			return values;
		}
		for (const [decl_id, ref] of Object.entries(key.easing.params)) {
			values[decl_id] = clone_param_value(ref.value);
		}
		return values;
	};

	const selected_keys_as_clipboard_entries = (): CurveClipboardKey[] => {
		const ordered_ids = selected_key_ids_ordered();
		if (ordered_ids.length === 0) {
			return [];
		}
		const ordered_keys = ordered_ids
			.map((key_id) => parsed_key_by_id.get(key_id))
			.filter((key): key is ParsedCurveKey => key !== undefined);
		const anchor = ordered_keys[0];
		if (!anchor) {
			return [];
		}
		return ordered_keys.map((key) => ({
			position_offset: key.position - anchor.position,
			value_offset: key.value - anchor.value,
			easing_values: easing_values_for_key(key)
		}));
	};

	const apply_clipboard_entries_at_anchor = async (
		entries: CurveClipboardKey[]
	): Promise<boolean> => {
		if (entries.length === 0) {
			return false;
		}
		const anchor = key_anchor_point();
		for (const entry of entries) {
			await add_key_at(anchor.position + entry.position_offset, anchor.value + entry.value_offset, {
				easing_values: entry.easing_values
			});
			await wait_pending_key_create();
		}
		return true;
	};

	const copy_selected_keys_to_clipboard = (): boolean => {
		const entries = selected_keys_as_clipboard_entries();
		curveClipboardState.sourceCurveNodeId = liveNode.node_id;
		curveClipboardState.keys = entries;
		return entries.length > 0;
	};

	const cut_selected_keys_to_clipboard = async (): Promise<boolean> => {
		const copied = copy_selected_keys_to_clipboard();
		if (!copied) {
			return false;
		}
		await remove_selected_keys();
		return true;
	};

	const duplicate_selected_keys = async (): Promise<boolean> => {
		return apply_clipboard_entries_at_anchor(selected_keys_as_clipboard_entries());
	};

	const paste_clipboard_keys = async (): Promise<boolean> => {
		return apply_clipboard_entries_at_anchor(curveClipboardState.keys);
	};

	const remove_selected_key = async (): Promise<void> => {
		await remove_selected_keys();
	};

	const axis_midpoint = (): { position: number; value: number } => {
		if (!interaction_transform) {
			if (active_curve_range_constraint) {
				return {
					position:
						(active_curve_range_constraint.x_min + active_curve_range_constraint.x_max) * 0.5,
					value: (active_curve_range_constraint.y_min + active_curve_range_constraint.y_max) * 0.5
				};
			}
			return { position: 0, value: 0 };
		}
		return {
			position: (interaction_transform.x_min + interaction_transform.x_max) * 0.5,
			value: (interaction_transform.y_min + interaction_transform.y_max) * 0.5
		};
	};

	$effect(() => {
		if (!curve_context_menu_open) {
			return;
		}
		if (curve_context_menu_anchor && curve_context_menu_items.length > 0) {
			return;
		}
		close_curve_context_menu();
	});

	$effect(() => {
		const unregisterHandlers = [
			registerCommandHandler(
				'edit.deleteSelection',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					return remove_selected_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'select.all',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					set_all_keys_selected();
					return true;
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.copy',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					copy_selected_keys_to_clipboard();
					return true;
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.cut',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					return cut_selected_keys_to_clipboard().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.duplicate',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					return duplicate_selected_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'edit.paste',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					return paste_clipboard_keys().then(() => true);
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'view.frame',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					return frame_selected_view();
				},
				{ priority: 200 }
			),
			registerCommandHandler(
				'view.home',
				() => {
					if (!is_canvas_focused) {
						return false;
					}
					home_view();
					return true;
				},
				{ priority: 200 }
			)
		];

		return () => {
			for (const unregister of unregisterHandlers) {
				unregister();
			}
		};
	});

	$effect(() => {
		return () => {
			if (drag_commit_raf_id !== 0 && typeof window !== 'undefined') {
				window.cancelAnimationFrame(drag_commit_raf_id);
				drag_commit_raf_id = 0;
			}
			cancel_view_bounds_animation();
			finish_drag();
			finish_canvas_pan();
			finish_curve_draw(undefined, false);
			finish_box_selection();
			const pending = pending_create_target;
			pending_create_target = null;
			if (pending?.edit_session) {
				void pending.edit_session.end();
			}
		};
	});
</script>

{#if liveNode.node_type === CURVE_NODE_TYPE}
	<div class="animation-curve-node-editor">
		<div class="curve-canvas-shell" style:height={canvasHeight}>
			<AnimationCurveCanvas
				compiledCurve={compiled_curve}
				parsedKeys={parsed_keys}
				selectedKeyId={selected_key_id}
				selectedKeyIds={selected_key_ids}
				selectedCurveOwnerKeyIds={selected_curve_owner_key_ids}
				hoverKeyId={hover_key_id}
				hoverCurveOwnerKeyId={hover_curve_owner_key_id}
				bezierHandles={bezier_handle_visuals}
				activeBezierHandle={active_bezier_handle_drag ? active_bezier_handle_drag.ref : null}
				hoverBezierHandle={hover_bezier_handle}
				hoverCurvePosition={hover_curve_position}
				drawPathPoints={active_curve_draw_points}
				selectionRect={active_box_selection_rect}
				activeCurveRangeConstraint={active_curve_range_constraint}
				fixedViewBounds={fixed_view_bounds}
				bind:interactionTransform={interaction_transform}
				bind:canvasElement={canvas_element}
				activeCanvasPan={active_canvas_pan !== null}
				{showGrid}
				{showNumbers}
				{showBounds}
				canvasHeight="100%"
				sampleCurveAt={(position) => sample_compiled_curve(compiled_curve, position)}
				onpointerdown={on_canvas_pointer_down}
				onpointermove={on_canvas_pointer_move}
				onpointerup={on_canvas_pointer_up}
				onpointercancel={on_canvas_pointer_cancel}
				onpointerleave={on_canvas_pointer_leave}
				onwheel={on_canvas_wheel}
				oncontextmenu={on_canvas_context_menu}
				onfocus={on_canvas_focus}
				onblur={on_canvas_blur} />
		</div>
		<ContextMenu
			bind:open={curve_context_menu_open}
			items={curve_context_menu_items}
			anchor={curve_context_menu_anchor}
			insideElements={[canvas_element]}
			minWidthRem={10}
			maxWidthCss="min(18rem, calc(100vw - 2rem))"
			zIndex={60}
			onClose={close_curve_context_menu} />
	</div>
{/if}

<style>
	.animation-curve-node-editor {
		display: flex;
		flex-direction: column;
		gap: 0.45rem;
		min-height: 0;
	}

	.curve-canvas-shell {
		height: min(26rem, 38vh);
		display: flex;
		flex-direction: column;
		min-height: 0;
		overflow: hidden;
	}
</style>
