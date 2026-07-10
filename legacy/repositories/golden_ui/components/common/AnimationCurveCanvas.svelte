<script lang="ts">
	import type { NodeId } from '../../types';

	interface ParsedCurveKeyLike {
		node_id: NodeId;
		position: number;
		value: number;
	}

	interface CompiledCurveKeyLike {
		node_id: NodeId;
		position: number;
		value: number;
	}

	interface CompiledCurveSegmentLike {
		start_position: number;
		end_position: number;
		start_value: number;
		end_value: number;
		easing:
			| { kind: 'linear' }
			| { kind: 'hold' }
			| { kind: 'bezier' }
			| { kind: 'shape' }
			| { kind: 'perlinNoise' }
			| { kind: 'elastic' }
			| { kind: 'bounce' }
			| { kind: 'random' }
			| { kind: 'script' }
			| { kind: 'steps'; steps: number };
	}

	interface CompiledCurveLike {
		keys: CompiledCurveKeyLike[];
		segments: CompiledCurveSegmentLike[];
	}

	interface CurveRangeConstraint {
		x_min: number;
		x_max: number;
		y_min: number;
		y_max: number;
	}

	interface CurveViewBounds {
		x_min: number;
		x_max: number;
		y_min: number;
		y_max: number;
	}

	interface CanvasSelectionRect {
		left: number;
		top: number;
		width: number;
		height: number;
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

	interface GridBlendStep {
		step: number;
		weight: number;
	}

	interface CurveCanvasTheme {
		canvas_bg: string;
		canvas_wrap_bg: string;
		canvas_wrap_border: string;
		line_idle: string;
		point_idle: string;
		fill_rgb: string;
		grid_line_rgb: string;
		grid_label_rgb: string;
		border_unbounded: string;
		border_bounded: string;
		empty_text: string;
		plot_fill: string;
		plot_shadow: string;
		plot_shadow_fill: string;
		hover_segment_underlay: string;
		hover_segment: string;
		selected_segment_underlay: string;
		selected_segment: string;
		handle_line: string;
		handle_line_aligned: string;
		handle_line_unlocked: string;
		handle_active: string;
		handle_hover: string;
		handle_out: string;
		handle_in: string;
		handle_stroke: string;
		handle_anchor_aligned: string;
		handle_anchor_unlocked: string;
		key_selected: string;
		key_hover: string;
		key_stroke: string;
		key_guide: string;
		draw_path_stroke: string;
		draw_path_point: string;
		crosshair: string;
		selection_fill: string;
		selection_stroke: string;
		bounds_readout: string;
	}

	type BezierHandleKind = 'out' | 'in';
	type BezierHandleAlignmentState = 'single' | 'aligned' | 'unlocked';

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

	interface BezierHandleRef {
		key_id: NodeId;
		kind: BezierHandleKind;
	}

	interface CurveDrawPathPoint {
		position: number;
		value: number;
	}

	const CURVE_EPSILON = 1e-12;
	const GRID_MANTISSAS = [1, 2, 5] as const;
	const GRID_MAX_LINES = 4000;
	const GRID_MIN_PIXEL_SPACING = 3.5;
	const CURVE_BOUNDS_SAMPLE_MIN = 192;
	const CURVE_BOUNDS_SAMPLE_MAX = 2048;
	const CURVE_RENDER_TARGET_SAMPLE_SPACING_PX = 3.25;
	const CURVE_RENDER_MAX_CHORD_ERROR_PX = 0.24;
	const CURVE_RENDER_MAX_SEGMENT_SAMPLES = 4096;
	const CURVE_RENDER_MAX_SUBDIVISION_DEPTH = 14;
	const CURVE_DRAW_MIN_DELTA_PX = 0.08;

	const clamp = (value: number, min: number, max: number): number =>
		Math.min(max, Math.max(min, value));
	const css_var_or = (
		styles: CSSStyleDeclaration,
		variable_name: string,
		fallback: string
	): string => {
		const value = styles.getPropertyValue(variable_name).trim();
		return value.length > 0 ? value : fallback;
	};
	const format_number = (value: number): string => {
		if (!Number.isFinite(value)) {
			return '0';
		}
		if (Math.abs(value) >= 1000 || (Math.abs(value) > 0 && Math.abs(value) < 0.001)) {
			return value.toExponential(2);
		}
		return value.toFixed(3).replace(/\.?0+$/, '');
	};
	const point_to_chord_distance_sq = (
		point_x: number,
		point_y: number,
		start_x: number,
		start_y: number,
		end_x: number,
		end_y: number
	): number => {
		const delta_x = end_x - start_x;
		const delta_y = end_y - start_y;
		const chord_length_sq = delta_x * delta_x + delta_y * delta_y;
		if (chord_length_sq <= CURVE_EPSILON) {
			const point_delta_x = point_x - start_x;
			const point_delta_y = point_y - start_y;
			return point_delta_x * point_delta_x + point_delta_y * point_delta_y;
		}
		const numerator = delta_x * (start_y - point_y) - (start_x - point_x) * delta_y;
		return (numerator * numerator) / chord_length_sq;
	};

	let {
		parsedKeys,
		compiledCurve,
		selectedKeyId,
		selectedKeyIds = [],
		selectedCurveOwnerKeyIds = [],
		hoverKeyId,
		hoverCurveOwnerKeyId = null,
		bezierHandles = [],
		activeBezierHandle = null,
		hoverBezierHandle = null,
		hoverCurvePosition,
		drawPathPoints = [],
		selectionRect = null,
		activeCurveRangeConstraint = null,
		fixedViewBounds = null,
		interactionTransform = $bindable<CanvasTransform | null>(null),
		canvasElement = $bindable<HTMLCanvasElement | null>(null),
		activeCanvasPan = false,
		showGrid = true,
		showNumbers = true,
		showBounds = true,
		canvasHeight = 'min(26rem, 38vh)',
		sampleCurveAt,
		onpointerdown,
		onpointermove,
		onpointerup,
		onpointercancel,
		onpointerleave,
		onwheel,
		oncontextmenu,
		onfocus,
		onblur
	}: {
		parsedKeys: ParsedCurveKeyLike[];
		compiledCurve: CompiledCurveLike;
		selectedKeyId: NodeId | null;
		selectedKeyIds?: NodeId[];
		selectedCurveOwnerKeyIds?: NodeId[];
		hoverKeyId: NodeId | null;
		hoverCurveOwnerKeyId?: NodeId | null;
		bezierHandles?: BezierHandleVisual[];
		activeBezierHandle?: BezierHandleRef | null;
		hoverBezierHandle?: BezierHandleRef | null;
		hoverCurvePosition: { position: number; value: number } | null;
		drawPathPoints?: CurveDrawPathPoint[];
		selectionRect?: CanvasSelectionRect | null;
		activeCurveRangeConstraint?: CurveRangeConstraint | null;
		fixedViewBounds?: CurveViewBounds | null;
		interactionTransform?: CanvasTransform | null;
		canvasElement?: HTMLCanvasElement | null;
		activeCanvasPan?: boolean;
		showGrid?: boolean;
		showNumbers?: boolean;
		showBounds?: boolean;
		canvasHeight?: string;
		sampleCurveAt: (position: number) => number | null;
		onpointerdown?: (event: PointerEvent) => void;
		onpointermove?: (event: PointerEvent) => void;
		onpointerup?: (event: PointerEvent) => void;
		onpointercancel?: (event: PointerEvent) => void;
		onpointerleave?: (event: PointerEvent) => void;
		onwheel?: (event: WheelEvent) => void;
		oncontextmenu?: (event: MouseEvent) => void;
		onfocus?: (event: FocusEvent) => void;
		onblur?: (event: FocusEvent) => void;
	} = $props();

	let rem_base_px = $state(16);
	let canvas_context = $state<CanvasRenderingContext2D | null>(null);
	let canvas_width_px = $state(1);
	let canvas_height_px = $state(1);
	let bounds_label = $state('Bounds : X [min: -, max: -] | Y [min: -, max: -]');
	let curve_canvas_theme = $state<CurveCanvasTheme>({
		canvas_bg: '#000000',
		canvas_wrap_bg: 'transparent',
		canvas_wrap_border: 'transparent',
		line_idle: '#9aa1ac',
		point_idle: '#b1bac9',
		fill_rgb: '80, 162, 255',
		grid_line_rgb: '164, 174, 191',
		grid_label_rgb: '186, 195, 210',
		border_unbounded: 'rgba(138, 147, 161, 0.46)',
		border_bounded: 'rgba(185, 199, 219, 0.82)',
		empty_text: 'rgba(192, 206, 232, 0.64)',
		plot_fill: 'rgba(0, 0, 0, 0.2)',
		plot_shadow: 'rgba(0, 0, 0, 0.5)',
		plot_shadow_fill: '#000000',
		hover_segment_underlay: 'rgba(92, 186, 255, 0.24)',
		hover_segment: 'rgba(146, 214, 255, 0.74)',
		selected_segment_underlay: 'rgba(92, 241, 170, 0.34)',
		selected_segment: 'rgba(150, 255, 203, 0.96)',
		handle_line: 'rgba(222, 232, 251, 0.32)',
		handle_line_aligned: 'rgba(120, 234, 198, 0.46)',
		handle_line_unlocked: 'rgba(255, 177, 118, 0.42)',
		handle_active: 'rgba(255, 224, 130, 0.98)',
		handle_hover: 'rgba(255, 199, 138, 0.95)',
		handle_out: 'rgba(172, 235, 220, 0.9)',
		handle_in: 'rgba(164, 205, 245, 0.9)',
		handle_stroke: 'rgba(10, 16, 24, 0.95)',
		handle_anchor_aligned: 'rgba(141, 245, 205, 0.94)',
		handle_anchor_unlocked: 'rgba(255, 183, 125, 0.94)',
		key_selected: 'rgba(255, 229, 112, 0.98)',
		key_hover: 'rgba(255, 182, 96, 0.95)',
		key_stroke: 'rgba(8, 12, 18, 0.9)',
		key_guide: 'rgba(255, 226, 117, 0.35)',
		draw_path_stroke: 'rgba(255, 140, 98, 0.96)',
		draw_path_point: 'rgba(255, 198, 122, 0.98)',
		crosshair: 'rgba(217, 231, 255, 0)',
		selection_fill: 'rgba(114, 210, 255, 0.14)',
		selection_stroke: 'rgba(158, 228, 255, 0.9)',
		bounds_readout: 'rgba(198, 208, 224, 0.86)'
	});

	const is_step_multiple = (value: number, step: number): boolean => {
		if (!(step > CURVE_EPSILON) || !Number.isFinite(value)) {
			return false;
		}
		const ratio = value / step;
		const rounded = Math.round(ratio);
		return Math.abs(ratio - rounded) <= 1e-6 * Math.max(1, Math.abs(ratio));
	};

	const resolve_grid_steps = (ideal_step: number): GridBlendStep[] => {
		if (!Number.isFinite(ideal_step) || ideal_step <= CURVE_EPSILON) {
			return [{ step: 1, weight: 1 }];
		}

		const exponent = Math.floor(Math.log10(ideal_step));
		const candidates: number[] = [];
		for (let level = exponent - 2; level <= exponent + 2; level += 1) {
			const decade = 10 ** level;
			for (const mantissa of GRID_MANTISSAS) {
				candidates.push(mantissa * decade);
			}
		}
		candidates.sort((left, right) => left - right);

		let lower = candidates[0];
		let upper = candidates[candidates.length - 1];
		for (const candidate of candidates) {
			if (candidate <= ideal_step) {
				lower = candidate;
			}
			if (candidate >= ideal_step) {
				upper = candidate;
				break;
			}
		}

		if (Math.abs(upper - lower) <= CURVE_EPSILON) {
			return [{ step: lower, weight: 1 }];
		}

		const log_lower = Math.log(lower);
		const log_upper = Math.log(upper);
		const span = log_upper - log_lower;
		if (Math.abs(span) <= CURVE_EPSILON) {
			return [{ step: lower, weight: 1 }];
		}
		const blend = clamp((Math.log(ideal_step) - log_lower) / span, 0, 1);
		const lower_weight = 1 - blend;
		const upper_weight = blend;
		const levels: GridBlendStep[] = [];
		if (lower_weight > 0.03) {
			levels.push({ step: lower, weight: lower_weight });
		}
		if (upper_weight > 0.03) {
			levels.push({ step: upper, weight: upper_weight });
		}
		if (levels.length === 0) {
			levels.push({ step: blend >= 0.5 ? upper : lower, weight: 1 });
		}
		return levels;
	};

	const draw_grid_lines = (
		context: CanvasRenderingContext2D,
		min_value: number,
		max_value: number,
		step: number,
		skip_step: number | null,
		alpha: number,
		line_width: number,
		to_screen: (value: number) => number,
		is_vertical: boolean,
		line_start: number,
		line_end: number
	): void => {
		if (
			alpha <= 0 ||
			!Number.isFinite(step) ||
			step <= CURVE_EPSILON ||
			!Number.isFinite(min_value) ||
			!Number.isFinite(max_value)
		) {
			return;
		}

		const start_index = Math.ceil((min_value - step * 1e-9) / step);
		const end_index = Math.floor((max_value + step * 1e-9) / step);
		const line_count = end_index - start_index + 1;
		if (line_count <= 0 || line_count > GRID_MAX_LINES) {
			return;
		}

		context.strokeStyle = `rgba(${curve_canvas_theme.grid_line_rgb}, ${alpha})`;
		context.lineWidth = line_width;
		context.beginPath();
		let has_line = false;

		for (let index = start_index; index <= end_index; index += 1) {
			const value = index * step;
			if (skip_step !== null && is_step_multiple(value, skip_step)) {
				continue;
			}

			const screen = to_screen(value);
			if (!Number.isFinite(screen)) {
				continue;
			}

			if (is_vertical) {
				context.moveTo(screen, line_start);
				context.lineTo(screen, line_end);
			} else {
				context.moveTo(line_start, screen);
				context.lineTo(line_end, screen);
			}
			has_line = true;
		}

		if (has_line) {
			context.stroke();
		}
	};

	const draw_axis_grid = (
		context: CanvasRenderingContext2D,
		min_value: number,
		max_value: number,
		axis_pixels: number,
		target_major_spacing_px: number,
		to_screen: (value: number) => number,
		is_vertical: boolean,
		line_start: number,
		line_end: number
	): void => {
		const span = max_value - min_value;
		if (!Number.isFinite(span) || span <= CURVE_EPSILON || axis_pixels <= CURVE_EPSILON) {
			return;
		}

		const ideal_major_step = (span * target_major_spacing_px) / Math.max(1, axis_pixels);
		const levels = resolve_grid_steps(Math.max(ideal_major_step, CURVE_EPSILON));
		for (const level of levels) {
			const major_step = level.step;
			const major_px = (major_step / span) * axis_pixels;
			if (!Number.isFinite(major_px) || major_px < GRID_MIN_PIXEL_SPACING) {
				continue;
			}

			const major_visibility = clamp(
				(major_px - GRID_MIN_PIXEL_SPACING) / (GRID_MIN_PIXEL_SPACING * 2.8),
				0,
				1
			);
			const major_alpha = (0.01 + 0.13 * level.weight) * major_visibility;
			draw_grid_lines(
				context,
				min_value,
				max_value,
				major_step,
				null,
				major_alpha,
				1,
				to_screen,
				is_vertical,
				line_start,
				line_end
			);

			const minor_step = major_step / 5;
			const minor_px = major_px / 5;
			if (minor_px >= GRID_MIN_PIXEL_SPACING * 1.25) {
				const minor_visibility = clamp(
					(minor_px - GRID_MIN_PIXEL_SPACING * 1.25) / (GRID_MIN_PIXEL_SPACING * 2.4),
					0,
					1
				);
				const minor_alpha = (0.012 + 0.064 * level.weight) * minor_visibility ** 1.8;
				draw_grid_lines(
					context,
					min_value,
					max_value,
					minor_step,
					major_step,
					minor_alpha,
					0.75,
					to_screen,
					is_vertical,
					line_start,
					line_end
				);
			}

			const micro_step = minor_step / 2;
			const micro_px = minor_px / 2;
			if (micro_px >= GRID_MIN_PIXEL_SPACING * 2.2) {
				const micro_visibility = clamp(
					(micro_px - GRID_MIN_PIXEL_SPACING * 2.2) / (GRID_MIN_PIXEL_SPACING * 2.2),
					0,
					1
				);
				const micro_alpha = (0.004 + 0.024 * level.weight) * micro_visibility ** 2.2;
				draw_grid_lines(
					context,
					min_value,
					max_value,
					micro_step,
					minor_step,
					micro_alpha,
					0.6,
					to_screen,
					is_vertical,
					line_start,
					line_end
				);
			}
		}
	};

	const draw_axis_tick_labels = (
		context: CanvasRenderingContext2D,
		min_value: number,
		max_value: number,
		axis_pixels: number,
		target_major_spacing_px: number,
		to_screen: (value: number) => number,
		axis: 'x' | 'y',
		plot_left: number,
		plot_top: number,
		plot_width: number,
		plot_height: number,
		font_px: number
	): void => {
		const span = max_value - min_value;
		if (
			!Number.isFinite(span) ||
			span <= CURVE_EPSILON ||
			!Number.isFinite(min_value) ||
			!Number.isFinite(max_value) ||
			axis_pixels <= CURVE_EPSILON
		) {
			return;
		}

		const safe_font = Math.max(7, font_px);
		context.font = `${safe_font}px sans-serif`;
		const ideal_major_step = (span * target_major_spacing_px) / Math.max(1, axis_pixels);
		const levels = resolve_grid_steps(Math.max(ideal_major_step, CURVE_EPSILON));

		if (axis === 'x') {
			const baseline_y = plot_top + plot_height + safe_font * 1.12;
			for (const level of levels) {
				const step = Math.max(CURVE_EPSILON, level.step);
				const step_px = (step / span) * axis_pixels;
				const label_visibility = clamp(
					(step_px - GRID_MIN_PIXEL_SPACING * 5.5) / (GRID_MIN_PIXEL_SPACING * 10.8),
					0,
					1
				);
				const label_alpha = (0.2 + 0.62 * level.weight) * label_visibility;
				if (label_alpha <= 0.02) {
					continue;
				}

				const start_index = Math.ceil((min_value - step * 1e-9) / step);
				const end_index = Math.floor((max_value + step * 1e-9) / step);
				const tick_count = end_index - start_index + 1;
				if (tick_count <= 0 || tick_count > 120) {
					continue;
				}

				context.fillStyle = `rgba(${curve_canvas_theme.grid_label_rgb}, ${label_alpha})`;
				let previous_right = Number.NEGATIVE_INFINITY;
				for (let index = start_index; index <= end_index; index += 1) {
					const value = index * step;
					const x = to_screen(value);
					if (x < plot_left || x > plot_left + plot_width) {
						continue;
					}
					const label = format_number(value);
					const text_width = context.measureText(label).width;
					const left = x - text_width * 0.5;
					const right = x + text_width * 0.5;
					if (left <= previous_right + safe_font * 0.55) {
						continue;
					}
					previous_right = right;
					context.fillText(label, left, baseline_y);
				}
			}
			return;
		}

		const label_right = plot_left - safe_font * 0.38;
		for (const level of levels) {
			const step = Math.max(CURVE_EPSILON, level.step);
			const step_px = (step / span) * axis_pixels;
			const label_visibility = clamp(
				(step_px - GRID_MIN_PIXEL_SPACING * 6.2) / (GRID_MIN_PIXEL_SPACING * 10.8),
				0,
				1
			);
			const label_alpha = (0.17 + 0.56 * level.weight) * label_visibility;
			if (label_alpha <= 0.02) {
				continue;
			}

			const start_index = Math.ceil((min_value - step * 1e-9) / step);
			const end_index = Math.floor((max_value + step * 1e-9) / step);
			const tick_count = end_index - start_index + 1;
			if (tick_count <= 0 || tick_count > 120) {
				continue;
			}

			context.fillStyle = `rgba(${curve_canvas_theme.grid_label_rgb}, ${label_alpha})`;
			let previous_y = Number.NEGATIVE_INFINITY;
			for (let index = end_index; index >= start_index; index -= 1) {
				const value = index * step;
				const y = to_screen(value);
				if (y < plot_top || y > plot_top + plot_height) {
					continue;
				}
				if (y - previous_y < safe_font * 1.12) {
					continue;
				}
				previous_y = y;
				const label = format_number(value);
				const text_width = context.measureText(label).width;
				context.fillText(label, label_right - text_width, y + safe_font * 0.31);
			}
		}
	};

	const draw_plot_border = (
		context: CanvasRenderingContext2D,
		plot_left: number,
		plot_top: number,
		plot_width: number,
		plot_height: number,
		border_radius: number,
		bounded_range: boolean
	): void => {
		const border_x = plot_left + 0.5;
		const border_y = plot_top + 0.5;
		const border_width = Math.max(0, plot_width - 1);
		const border_height = Math.max(0, plot_height - 1);
		if (!(border_width > 0) || !(border_height > 0)) {
			return;
		}

		context.save();
		context.strokeStyle = bounded_range
			? curve_canvas_theme.border_bounded
			: curve_canvas_theme.border_unbounded;
		context.lineWidth = bounded_range ? 1.15 : 0.95;
		if (bounded_range) {
			context.setLineDash([]);
		} else {
			const dash = Math.max(2, rem_base_px * 0.14);
			context.setLineDash([dash, dash * 0.9]);
		}
		context.beginPath();
		context.roundRect(border_x, border_y, border_width, border_height, border_radius);
		context.stroke();
		context.restore();
	};

	const build_curve_samples = (
		x_min: number,
		x_max: number,
		plot_width: number,
		compiled_curve: CompiledCurveLike,
		to_screen_x: (position: number) => number,
		to_screen_y: (value: number) => number,
		sample_value: (position: number) => number
	): {
		positions: Float64Array;
		values: Float64Array;
		screen_x: Float64Array;
		screen_y: Float64Array;
		edge_owner_key_ids: Array<NodeId | null>;
	} => {
		const sample_cache = new Map<number, number>();
		type SampledCurvePoint = {
			position: number;
			value: number;
			screen_x: number;
			screen_y: number;
		};
		const sampled_point_cache = new Map<number, SampledCurvePoint>();
		const sample_at = (position: number): number => {
			const cached = sample_cache.get(position);
			if (cached !== undefined) {
				return cached;
			}
			const sampled = sample_value(position);
			sample_cache.set(position, sampled);
			return sampled;
		};
		const sample_point = (position: number): SampledCurvePoint => {
			const cached = sampled_point_cache.get(position);
			if (cached) {
				return cached;
			}
			const sampled: SampledCurvePoint = {
				position,
				value: sample_at(position),
				screen_x: to_screen_x(position),
				screen_y: to_screen_y(sample_at(position))
			};
			sampled_point_cache.set(position, sampled);
			return sampled;
		};

		const positions: number[] = [];
		const values: number[] = [];
		const screen_x: number[] = [];
		const screen_y: number[] = [];
		const edge_owner_key_ids: Array<NodeId | null> = [];
		const step_value_at_index = (
			segment: CompiledCurveSegmentLike,
			steps: number,
			step_index: number
		): number => {
			const stepped_progress = clamp(step_index / Math.max(1, steps), 0, 1);
			return segment.start_value + (segment.end_value - segment.start_value) * stepped_progress;
		};
		const append_point = (
			position: number,
			value: number,
			edge_owner_key_id: NodeId | null
		): boolean => {
			const last_index = positions.length - 1;
			if (
				last_index >= 0 &&
				Math.abs(positions[last_index] - position) <= CURVE_EPSILON &&
				Math.abs(values[last_index] - value) <= CURVE_EPSILON
			) {
				positions[last_index] = position;
				values[last_index] = value;
				return false;
			}
			positions.push(position);
			values.push(value);
			screen_x.push(to_screen_x(position));
			screen_y.push(to_screen_y(value));
			if (positions.length > 1) {
				edge_owner_key_ids.push(edge_owner_key_id);
			}
			return true;
		};
		const append_sampled_point = (
			point: SampledCurvePoint,
			edge_owner_key_id: NodeId | null
		): boolean => {
			const last_index = positions.length - 1;
			if (
				last_index >= 0 &&
				Math.abs(positions[last_index] - point.position) <= CURVE_EPSILON &&
				Math.abs(values[last_index] - point.value) <= CURVE_EPSILON
			) {
				positions[last_index] = point.position;
				values[last_index] = point.value;
				screen_x[last_index] = point.screen_x;
				screen_y[last_index] = point.screen_y;
				return false;
			}
			positions.push(point.position);
			values.push(point.value);
			screen_x.push(point.screen_x);
			screen_y.push(point.screen_y);
			if (positions.length > 1) {
				edge_owner_key_ids.push(edge_owner_key_id);
			}
			return true;
		};

		const keys = compiled_curve.keys;
		const segments = compiled_curve.segments;
		if (segments.length === 0) {
			append_point(x_min, sample_at(x_min), null);
			append_point(x_max, sample_at(x_max), null);
			return {
				positions: Float64Array.from(positions),
				values: Float64Array.from(values),
				screen_x: Float64Array.from(screen_x),
				screen_y: Float64Array.from(screen_y),
				edge_owner_key_ids
			};
		}

		const first_key = keys[0];
		const last_key = keys[keys.length - 1];
		if (first_key && x_min < first_key.position - CURVE_EPSILON) {
			append_point(x_min, sample_at(x_min), null);
			append_point(Math.min(x_max, first_key.position), first_key.value, null);
		}

		for (let segment_index = 0; segment_index < segments.length; segment_index += 1) {
			const segment = segments[segment_index];
			const owner_key_id = keys[segment_index]?.node_id ?? null;
			const visible_start = Math.max(x_min, segment.start_position);
			const visible_end = Math.min(x_max, segment.end_position);
			if (!(visible_end > visible_start + CURVE_EPSILON) || owner_key_id === null) {
				continue;
			}

			const span = Math.max(CURVE_EPSILON, segment.end_position - segment.start_position);
			const epsilon = Math.max(CURVE_EPSILON, span * 1e-9);
			let segment_point_count = 0;
			const append_segment_point = (point: SampledCurvePoint): void => {
				if (append_sampled_point(point, owner_key_id)) {
					segment_point_count += 1;
				}
			};
			const visible_start_point = sample_point(visible_start);
			const visible_end_point = sample_point(visible_end);
			append_segment_point(visible_start_point);

			if (segment.easing.kind === 'hold') {
				append_point(visible_end, segment.start_value, owner_key_id);
				if (visible_end >= segment.end_position - CURVE_EPSILON) {
					append_point(segment.end_position, segment.end_value, owner_key_id);
				}
				continue;
			}

			if (segment.easing.kind === 'steps') {
				const steps = Math.max(1, Math.round(segment.easing.steps));
				for (let step_index = 1; step_index <= steps; step_index += 1) {
					const boundary_position =
						segment.start_position + (span * step_index) / Math.max(1, steps);
					if (
						boundary_position <= visible_start + epsilon ||
						boundary_position > visible_end + epsilon
					) {
						continue;
					}
					append_point(
						boundary_position,
						step_value_at_index(segment, steps, step_index - 1),
						owner_key_id
					);
					append_point(
						boundary_position,
						step_value_at_index(segment, steps, step_index),
						owner_key_id
					);
				}
				append_point(visible_end, sample_at(visible_end), owner_key_id);
				continue;
			}

			if (segment.easing.kind === 'linear') {
				append_segment_point(visible_end_point);
				continue;
			}

			const append_adaptive_segment = (
				start_point: SampledCurvePoint,
				end_point: SampledCurvePoint,
				depth: number
			): void => {
				if (segment_point_count >= CURVE_RENDER_MAX_SEGMENT_SAMPLES) {
					append_segment_point(end_point);
					return;
				}

				const position_span = end_point.position - start_point.position;
				if (
					depth >= CURVE_RENDER_MAX_SUBDIVISION_DEPTH ||
					!Number.isFinite(position_span) ||
					position_span <= CURVE_EPSILON
				) {
					append_segment_point(end_point);
					return;
				}

				const quarter_point = sample_point(start_point.position + position_span * 0.25);
				const midpoint = sample_point(start_point.position + position_span * 0.5);
				const three_quarter_point = sample_point(start_point.position + position_span * 0.75);
				const chord_error_sq = Math.max(
					point_to_chord_distance_sq(
						quarter_point.screen_x,
						quarter_point.screen_y,
						start_point.screen_x,
						start_point.screen_y,
						end_point.screen_x,
						end_point.screen_y
					),
					point_to_chord_distance_sq(
						midpoint.screen_x,
						midpoint.screen_y,
						start_point.screen_x,
						start_point.screen_y,
						end_point.screen_x,
						end_point.screen_y
					),
					point_to_chord_distance_sq(
						three_quarter_point.screen_x,
						three_quarter_point.screen_y,
						start_point.screen_x,
						start_point.screen_y,
						end_point.screen_x,
						end_point.screen_y
					)
				);
				const screen_span_x = Math.abs(end_point.screen_x - start_point.screen_x);
				const needs_subdivision =
					screen_span_x > CURVE_RENDER_TARGET_SAMPLE_SPACING_PX ||
					chord_error_sq > CURVE_RENDER_MAX_CHORD_ERROR_PX * CURVE_RENDER_MAX_CHORD_ERROR_PX;
				if (!needs_subdivision) {
					append_segment_point(end_point);
					return;
				}

				append_adaptive_segment(start_point, midpoint, depth + 1);
				append_adaptive_segment(midpoint, end_point, depth + 1);
			};

			append_adaptive_segment(visible_start_point, visible_end_point, 0);
		}

		if (last_key && x_max > last_key.position + CURVE_EPSILON) {
			append_point(Math.max(x_min, last_key.position), last_key.value, null);
			append_point(x_max, sample_at(x_max), null);
		}

		return {
			positions: Float64Array.from(positions),
			values: Float64Array.from(values),
			screen_x: Float64Array.from(screen_x),
			screen_y: Float64Array.from(screen_y),
			edge_owner_key_ids
		};
	};

	const draw_curve_canvas = (): void => {
		if (!canvas_context) {
			return;
		}
		const context = canvas_context;
		const width = Math.max(1, canvas_width_px);
		const height = Math.max(1, canvas_height_px);

		context.clearRect(0, 0, width, height);

		const keys = compiledCurve.keys;
		if (keys.length === 0) {
			context.fillStyle = curve_canvas_theme.empty_text;
			context.font = `${Math.max(9, Math.round(height * 0.065))}px sans-serif`;
			context.fillText('No keys in this curve', width * 0.04, height * 0.18);
			if (interactionTransform !== null) {
				interactionTransform = null;
			}
			const empty_bounds_label = 'Bounds : X [min: -, max: -] | Y [min: -, max: -]';
			if (bounds_label !== empty_bounds_label) {
				bounds_label = empty_bounds_label;
			}
			return;
		}

		const pad_left = Math.max(width * 0.03, rem_base_px * 1.8);
		const pad_right = Math.max(width * 0.03, rem_base_px * 0.8);
		const pad_top = Math.max(height * 0.06, rem_base_px * 0.7);
		const pad_bottom = Math.max(height * 0.03, rem_base_px * 1.3);
		const plot_width = Math.max(1, width - pad_left - pad_right);
		const plot_height = Math.max(1, height - pad_top - pad_bottom);
		const plot_left = pad_left;
		const plot_top = pad_top;

		const active_range = activeCurveRangeConstraint;
		let x_min = active_range ? active_range.x_min : keys[0].position;
		let x_max = active_range ? active_range.x_max : keys[keys.length - 1].position;
		if (!Number.isFinite(x_min) || !Number.isFinite(x_max)) {
			x_min = 0;
			x_max = 1;
		}
		if (Math.abs(x_max - x_min) <= CURVE_EPSILON) {
			if (active_range) {
				x_min = active_range.x_min;
				x_max = active_range.x_max;
			} else {
				x_min -= 0.5;
				x_max += 0.5;
			}
		}

		const bounds_sample_count = Math.max(
			CURVE_BOUNDS_SAMPLE_MIN,
			Math.min(CURVE_BOUNDS_SAMPLE_MAX, Math.round(plot_width * 1.2))
		);
		let bounds_sample_step = (x_max - x_min) / Math.max(1, bounds_sample_count - 1);
		let y_min = Number.POSITIVE_INFINITY;
		let y_max = Number.NEGATIVE_INFINITY;

		for (let index = 0; index < bounds_sample_count; index += 1) {
			const position =
				index + 1 === bounds_sample_count ? x_max : x_min + bounds_sample_step * index;
			const sample = sampleCurveAt(position);
			let value = sample ?? 0;
			if (active_range) {
				value = clamp(value, active_range.y_min, active_range.y_max);
			}
			if (value < y_min) {
				y_min = value;
			}
			if (value > y_max) {
				y_max = value;
			}
		}

		for (const key of parsedKeys) {
			const key_value = active_range
				? clamp(key.value, active_range.y_min, active_range.y_max)
				: key.value;
			if (key_value < y_min) {
				y_min = key_value;
			}
			if (key_value > y_max) {
				y_max = key_value;
			}
		}
		if (active_range) {
			y_min = active_range.y_min;
			y_max = active_range.y_max;
		} else {
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
			y_min -= value_pad;
			y_max += value_pad;
		}

		const fallback_view_bounds: CurveViewBounds = active_range
			? {
					x_min: active_range.x_min,
					x_max: active_range.x_max,
					y_min: active_range.y_min,
					y_max: active_range.y_max
				}
			: { x_min, x_max, y_min, y_max };
		const view_bounds = fixedViewBounds ?? fallback_view_bounds;
		x_min = view_bounds.x_min;
		x_max = view_bounds.x_max;
		y_min = view_bounds.y_min;
		y_max = view_bounds.y_max;

		const x_span = Math.max(CURVE_EPSILON, x_max - x_min);
		const y_span = Math.max(CURVE_EPSILON, y_max - y_min);
		const to_screen_x = (position: number): number =>
			plot_left + ((position - x_min) / x_span) * plot_width;
		const to_screen_y = (value: number): number =>
			plot_top + plot_height - ((value - y_min) / y_span) * plot_height;

		const sampled_curve = build_curve_samples(
			x_min,
			x_max,
			plot_width,
			compiledCurve,
			to_screen_x,
			to_screen_y,
			(position) => {
				const sample = sampleCurveAt(position);
				let value = sample ?? 0;
				if (active_range) {
					value = clamp(value, active_range.y_min, active_range.y_max);
				}
				return value;
			}
		);
		const sample_positions = sampled_curve.positions;
		const sample_screen_x = sampled_curve.screen_x;
		const sample_screen_y = sampled_curve.screen_y;
		const sample_count = sample_positions.length;
		const sample_owner_key_ids = sampled_curve.edge_owner_key_ids;

		const border_radius = rem_base_px * 0.3;
		context.save();
		context.beginPath();
		context.roundRect(plot_left, plot_top, plot_width, plot_height, border_radius);
		context.fillStyle = curve_canvas_theme.plot_fill;
		context.fill();
		context.clip();

		context.beginPath();
		context.rect(plot_left - 10, plot_top - 10, plot_width + 20, plot_height + 20);
		context.rect(plot_left, plot_top, plot_width, plot_height);
		context.shadowColor = curve_canvas_theme.plot_shadow;
		context.shadowBlur = 30; // How soft the shadow is
		context.shadowOffsetX = 0; // Shift shadow X (0 makes it even on all sides)
		context.shadowOffsetY = 0; // Shift shadow Y
		context.fillStyle = curve_canvas_theme.plot_shadow_fill;
		context.fill('evenodd');

		const target_major_spacing_px = Math.max(34, rem_base_px * 5.8);
		if (showGrid) {
			draw_axis_grid(
				context,
				x_min,
				x_max,
				plot_width,
				target_major_spacing_px,
				to_screen_x,
				true,
				plot_top,
				plot_top + plot_height
			);
			draw_axis_grid(
				context,
				y_min,
				y_max,
				plot_height,
				target_major_spacing_px,
				to_screen_y,
				false,
				plot_left,
				plot_left + plot_width
			);

			if (x_min <= 0 && x_max >= 0) {
				const x0 = to_screen_x(0);
				context.strokeStyle = `rgba(${curve_canvas_theme.grid_label_rgb}, 0.22)`;
				context.lineWidth = 1.05;
				context.beginPath();
				context.moveTo(x0, plot_top);
				context.lineTo(x0, plot_top + plot_height);
				context.stroke();
			}
			if (y_min <= 0 && y_max >= 0) {
				const y0 = to_screen_y(0);
				context.strokeStyle = `rgba(${curve_canvas_theme.grid_label_rgb}, 0.22)`;
				context.lineWidth = 1.05;
				context.beginPath();
				context.moveTo(plot_left, y0);
				context.lineTo(plot_left + plot_width, y0);
				context.stroke();
			}
		}

		context.restore();

		context.save();
		context.beginPath();
		context.roundRect(plot_left - 2, plot_top - 2, plot_width + 4, plot_height + 4, border_radius);
		context.clip();

		const zero_axis_y = clamp(to_screen_y(0), plot_top, plot_top + plot_height);
		context.beginPath();
		let has_area_fill_path = false;
		let first_area_x = 0;
		let last_area_x = 0;
		let last_area_y = 0;
		for (let index = 0; index < sample_count; index += 1) {
			const x = sample_screen_x[index];
			const y = sample_screen_y[index];
			if (!has_area_fill_path) {
				context.moveTo(x, y);
				has_area_fill_path = true;
				first_area_x = x;
				last_area_x = x;
				last_area_y = y;
				continue;
			}
			if (
				Math.abs(x - last_area_x) < CURVE_DRAW_MIN_DELTA_PX &&
				Math.abs(y - last_area_y) < CURVE_DRAW_MIN_DELTA_PX
			) {
				continue;
			}
			context.lineTo(x, y);
			last_area_x = x;
			last_area_y = y;
		}
		if (has_area_fill_path) {
			context.lineTo(last_area_x, zero_axis_y);
			context.lineTo(first_area_x, zero_axis_y);
			context.closePath();
			const zero_axis_ratio = clamp(
				(zero_axis_y - plot_top) / Math.max(CURVE_EPSILON, plot_height),
				0,
				1
			);
			const axis_blend = 0.05;
			const area_gradient = context.createLinearGradient(0, plot_top, 0, plot_top + plot_height);
			area_gradient.addColorStop(0, `rgba(${curve_canvas_theme.fill_rgb}, 0.25)`);
			area_gradient.addColorStop(
				Math.max(0, zero_axis_ratio - axis_blend),
				`rgba(${curve_canvas_theme.fill_rgb}, 0.05)`
			);
			area_gradient.addColorStop(zero_axis_ratio, `rgba(${curve_canvas_theme.fill_rgb}, 0)`);
			area_gradient.addColorStop(
				Math.min(1, zero_axis_ratio + axis_blend),
				`rgba(${curve_canvas_theme.fill_rgb}, 0.05)`
			);
			area_gradient.addColorStop(1, `rgba(${curve_canvas_theme.fill_rgb}, 0.25)`);
			context.fillStyle = area_gradient;
			context.fill();
		}

		context.strokeStyle = curve_canvas_theme.line_idle;
		context.lineWidth = 0.5;
		context.lineJoin = 'round';
		context.lineCap = 'round';

		const stroke_sample_range = (
			start_index: number,
			end_index: number,
			stroke_style: string,
			line_width: number,
			line_dash: number[] = [],
			alpha = 1
		): boolean => {
			if (end_index <= start_index) {
				return false;
			}
			context.save();
			context.strokeStyle = stroke_style;
			context.lineWidth = line_width;
			context.globalAlpha = alpha;
			context.setLineDash(line_dash);
			context.beginPath();
			let has_path = false;
			let last_x = 0;
			let last_y = 0;
			for (let index = start_index; index <= end_index; index += 1) {
				const x = sample_screen_x[index];
				const y = sample_screen_y[index];
				if (!has_path) {
					context.moveTo(x, y);
					has_path = true;
					last_x = x;
					last_y = y;
					continue;
				}
				if (
					Math.abs(x - last_x) < CURVE_DRAW_MIN_DELTA_PX &&
					Math.abs(y - last_y) < CURVE_DRAW_MIN_DELTA_PX
				) {
					continue;
				}
				context.lineTo(x, y);
				last_x = x;
				last_y = y;
			}
			if (has_path) {
				context.stroke();
			}
			context.restore();
			return has_path;
		};

		const first_key_position = keys[0]?.position ?? sample_positions[0] ?? 0;
		const last_key_position = keys[keys.length - 1]?.position ?? first_key_position;
		let first_main_sample_index = 0;
		while (
			first_main_sample_index < sample_count &&
			sample_positions[first_main_sample_index] < first_key_position - CURVE_EPSILON
		) {
			first_main_sample_index += 1;
		}
		let last_main_sample_index = sample_count - 1;
		while (
			last_main_sample_index >= 0 &&
			sample_positions[last_main_sample_index] > last_key_position + CURVE_EPSILON
		) {
			last_main_sample_index -= 1;
		}
		const max_sample_index = Math.max(0, sample_count - 1);
		const clamped_first_main_sample_index = clamp(first_main_sample_index, 0, max_sample_index);
		const clamped_last_main_sample_index = clamp(last_main_sample_index, 0, max_sample_index);
		const has_main_curve_range =
			first_main_sample_index < sample_count &&
			last_main_sample_index >= 0 &&
			clamped_first_main_sample_index <= clamped_last_main_sample_index;
		const dashed_line_pattern = [Math.max(3, rem_base_px * 0.18), Math.max(2, rem_base_px * 0.14)];

		if (first_main_sample_index > 0) {
			stroke_sample_range(
				0,
				clamped_first_main_sample_index,
				curve_canvas_theme.line_idle,
				0.5,
				dashed_line_pattern,
				0.8
			);
		}
		if (has_main_curve_range) {
			stroke_sample_range(
				clamped_first_main_sample_index,
				clamped_last_main_sample_index,
				curve_canvas_theme.line_idle,
				0.5
			);
		}
		if (last_main_sample_index + 1 < sample_count) {
			stroke_sample_range(
				clamped_last_main_sample_index,
				sample_count - 1,
				curve_canvas_theme.line_idle,
				0.5,
				dashed_line_pattern,
				0.8
			);
		}

		const stroke_owned_segment = (
			key_id: NodeId,
			stroke_style: string,
			line_width: number
		): boolean => {
			context.strokeStyle = stroke_style;
			context.lineWidth = line_width;
			context.beginPath();
			let drawing = false;
			let has_path = false;
			let last_x = 0;
			let last_y = 0;

			for (let index = 0; index + 1 < sample_count; index += 1) {
				if (sample_owner_key_ids[index] !== key_id) {
					drawing = false;
					continue;
				}

				const start_x = sample_screen_x[index];
				const start_y = sample_screen_y[index];
				const end_x = sample_screen_x[index + 1];
				const end_y = sample_screen_y[index + 1];
				if (!drawing) {
					context.moveTo(start_x, start_y);
					drawing = true;
					has_path = true;
					last_x = start_x;
					last_y = start_y;
				}
				if (
					Math.abs(end_x - last_x) < CURVE_DRAW_MIN_DELTA_PX &&
					Math.abs(end_y - last_y) < CURVE_DRAW_MIN_DELTA_PX
				) {
					continue;
				}
				context.lineTo(end_x, end_y);
				last_x = end_x;
				last_y = end_y;
			}

			if (has_path) {
				context.stroke();
			}
			return has_path;
		};

		const selected_key_id_list =
			selectedKeyIds.length > 0 ? selectedKeyIds : selectedKeyId === null ? [] : [selectedKeyId];
		const selected_key_set = new Set<NodeId>(selected_key_id_list);
		const selected_curve_owner_key_set = new Set<NodeId>(selectedCurveOwnerKeyIds);

		if (hoverCurveOwnerKeyId !== null && !selected_curve_owner_key_set.has(hoverCurveOwnerKeyId)) {
			if (
				stroke_owned_segment(hoverCurveOwnerKeyId, curve_canvas_theme.hover_segment_underlay, 2.55)
			) {
				stroke_owned_segment(hoverCurveOwnerKeyId, curve_canvas_theme.hover_segment, 1.34);
			}
		}

		for (const curve_owner_key_id of selectedCurveOwnerKeyIds) {
			if (
				stroke_owned_segment(curve_owner_key_id, curve_canvas_theme.selected_segment_underlay, 3.1)
			) {
				stroke_owned_segment(curve_owner_key_id, curve_canvas_theme.selected_segment, 1.76);
			}
		}

		if (drawPathPoints.length > 0) {
			context.save();
			context.strokeStyle = curve_canvas_theme.draw_path_stroke;
			context.lineWidth = 1.45;
			context.lineJoin = 'round';
			context.lineCap = 'round';
			context.beginPath();
			let has_draw_path = false;
			let last_x = 0;
			let last_y = 0;
			for (const point of drawPathPoints) {
				const x = to_screen_x(point.position);
				const y = to_screen_y(point.value);
				if (!Number.isFinite(x) || !Number.isFinite(y)) {
					continue;
				}
				if (!has_draw_path) {
					context.moveTo(x, y);
					has_draw_path = true;
					last_x = x;
					last_y = y;
					continue;
				}
				if (
					Math.abs(x - last_x) < CURVE_DRAW_MIN_DELTA_PX &&
					Math.abs(y - last_y) < CURVE_DRAW_MIN_DELTA_PX
				) {
					continue;
				}
				context.lineTo(x, y);
				last_x = x;
				last_y = y;
			}
			if (has_draw_path) {
				context.stroke();
				const first = drawPathPoints[0];
				const last = drawPathPoints[drawPathPoints.length - 1];
				for (const point of [first, last]) {
					const x = to_screen_x(point.position);
					const y = to_screen_y(point.value);
					context.fillStyle = curve_canvas_theme.draw_path_point;
					context.beginPath();
					context.arc(x, y, Math.max(2, rem_base_px * 0.12), 0, Math.PI * 2);
					context.fill();
				}
			}
			context.restore();
		}

		const drawn_handle_anchor_keys = new Set<NodeId>();
		for (const handle of bezierHandles) {
			const anchor_x = to_screen_x(handle.anchor_position);
			const anchor_y = to_screen_y(handle.anchor_value);
			const handle_x = to_screen_x(handle.handle_position);
			const handle_y = to_screen_y(handle.handle_value);
			const handle_line_color =
				handle.alignment_state === 'aligned'
					? curve_canvas_theme.handle_line_aligned
					: handle.alignment_state === 'unlocked'
						? curve_canvas_theme.handle_line_unlocked
						: curve_canvas_theme.handle_line;
			const handle_anchor_color =
				handle.alignment_state === 'aligned'
					? curve_canvas_theme.handle_anchor_aligned
					: handle.alignment_state === 'unlocked'
						? curve_canvas_theme.handle_anchor_unlocked
						: null;

			context.save();
			context.strokeStyle = handle_line_color;
			context.lineWidth = handle.alignment_state === 'unlocked' ? 1 : 0.9;
			if (handle.alignment_state === 'unlocked') {
				context.setLineDash([Math.max(2, rem_base_px * 0.16), Math.max(2, rem_base_px * 0.12)]);
			}
			context.beginPath();
			context.moveTo(anchor_x, anchor_y);
			context.lineTo(handle_x, handle_y);
			context.stroke();
			context.restore();

			if (handle_anchor_color !== null && !drawn_handle_anchor_keys.has(handle.anchor_key_id)) {
				drawn_handle_anchor_keys.add(handle.anchor_key_id);
				context.strokeStyle = handle_anchor_color;
				context.lineWidth = 1.05;
				context.beginPath();
				context.arc(anchor_x, anchor_y, Math.max(2.1, rem_base_px * 0.13), 0, Math.PI * 2);
				context.stroke();
			}

			const active =
				activeBezierHandle &&
				activeBezierHandle.key_id === handle.key_id &&
				activeBezierHandle.kind === handle.kind;
			const hovered =
				hoverBezierHandle &&
				hoverBezierHandle.key_id === handle.key_id &&
				hoverBezierHandle.kind === handle.kind;
			const radius = active ? Math.max(2.4, rem_base_px * 0.2) : Math.max(2.1, rem_base_px * 0.17);
			context.fillStyle = active
				? curve_canvas_theme.handle_active
				: hovered
					? curve_canvas_theme.handle_hover
					: handle.kind === 'out'
						? curve_canvas_theme.handle_out
						: curve_canvas_theme.handle_in;
			context.strokeStyle = curve_canvas_theme.handle_stroke;
			context.lineWidth = active ? 1.2 : 1;
			context.beginPath();
			context.arc(handle_x, handle_y, radius, 0, Math.PI * 2);
			context.fill();
			context.stroke();
		}

		const key_screen_points = new Map<NodeId, { x: number; y: number }>();
		const key_stride = parsedKeys.length > 2200 ? Math.ceil(parsedKeys.length / 2200) : 1;
		for (let index = 0; index < parsedKeys.length; index += 1) {
			const key = parsedKeys[index];
			const key_position = active_range
				? clamp(key.position, active_range.x_min, active_range.x_max)
				: key.position;
			const key_value = active_range
				? clamp(key.value, active_range.y_min, active_range.y_max)
				: key.value;
			const x = to_screen_x(key_position);
			const y = to_screen_y(key_value);
			key_screen_points.set(key.node_id, { x, y });

			const selected = selected_key_set.has(key.node_id);
			const hovered = key.node_id === hoverKeyId;
			if (!selected && !hovered && index % key_stride !== 0) {
				continue;
			}

			const radius = selected ? Math.max(2, rem_base_px * 0.27) : Math.max(1.5, rem_base_px * 0.2);
			context.fillStyle = selected
				? curve_canvas_theme.key_selected
				: hovered
					? curve_canvas_theme.key_hover
					: curve_canvas_theme.point_idle;
			context.strokeStyle = curve_canvas_theme.key_stroke;
			context.lineWidth = selected ? 1.25 : 1;
			context.beginPath();
			context.arc(x, y, radius, 0, Math.PI * 2);
			context.fill();
			context.stroke();
		}

		for (const selected_key_id of selected_key_id_list) {
			const selected_point = key_screen_points.get(selected_key_id);
			if (selected_point) {
				context.strokeStyle = curve_canvas_theme.key_guide;
				context.lineWidth = 1;
				context.beginPath();
				context.moveTo(selected_point.x, plot_top);
				context.lineTo(selected_point.x, plot_top + plot_height);
				context.stroke();
			}
		}

		if (hoverCurvePosition) {
			const cross_x = to_screen_x(hoverCurvePosition.position);
			const cross_y = to_screen_y(hoverCurvePosition.value);
			if (
				cross_x >= plot_left &&
				cross_x <= plot_left + plot_width &&
				cross_y >= plot_top &&
				cross_y <= plot_top + plot_height
			) {
				context.setLineDash([Math.max(2, rem_base_px * 0.12), Math.max(2, rem_base_px * 0.16)]);
				context.strokeStyle = curve_canvas_theme.crosshair;
				context.lineWidth = 0.9;
				context.beginPath();
				context.moveTo(cross_x, plot_top);
				context.lineTo(cross_x, plot_top + plot_height);
				context.moveTo(plot_left, cross_y);
				context.lineTo(plot_left + plot_width, cross_y);
				context.stroke();
				context.setLineDash([]);
			}
		}

		context.restore();

		if (selectionRect && selectionRect.width > 0 && selectionRect.height > 0) {
			const box_left = clamp(selectionRect.left, 0, width);
			const box_top = clamp(selectionRect.top, 0, height);
			const box_right = clamp(selectionRect.left + selectionRect.width, 0, width);
			const box_bottom = clamp(selectionRect.top + selectionRect.height, 0, height);
			const box_width = Math.max(0, box_right - box_left);
			const box_height = Math.max(0, box_bottom - box_top);
			if (box_width > 0 && box_height > 0) {
				context.fillStyle = curve_canvas_theme.selection_fill;
				context.strokeStyle = curve_canvas_theme.selection_stroke;
				context.lineWidth = 1.05;
				context.setLineDash([Math.max(2, rem_base_px * 0.11), Math.max(2, rem_base_px * 0.14)]);
				context.fillRect(box_left, box_top, box_width, box_height);
				context.strokeRect(box_left, box_top, box_width, box_height);
				context.setLineDash([]);
			}
		}

		if (showNumbers) {
			const axis_label_font_px = Math.max(7, Math.round(height * 0.032));
			draw_axis_tick_labels(
				context,
				x_min,
				x_max,
				plot_width,
				target_major_spacing_px,
				to_screen_x,
				'x',
				plot_left,
				plot_top,
				plot_width,
				plot_height,
				axis_label_font_px
			);
			draw_axis_tick_labels(
				context,
				y_min,
				y_max,
				plot_height,
				target_major_spacing_px,
				to_screen_y,
				'y',
				plot_left,
				plot_top,
				plot_width,
				plot_height,
				axis_label_font_px
			);
		}

		if (showGrid) {
			draw_plot_border(
				context,
				plot_left,
				plot_top,
				plot_width,
				plot_height,
				border_radius,
				Boolean(active_range)
			);
		}

		const next_bounds_label = `Bounds : X [min: ${format_number(x_min)}, max: ${format_number(x_max)}] | Y [min: ${format_number(y_min)}, max: ${format_number(y_max)}]`;
		if (bounds_label !== next_bounds_label) {
			bounds_label = next_bounds_label;
		}

		const next_transform: CanvasTransform = {
			plot_left,
			plot_top,
			plot_width,
			plot_height,
			x_min,
			x_max,
			y_min,
			y_max,
			key_screen_points,
			curve_screen_x: sample_screen_x,
			curve_screen_y: sample_screen_y,
			curve_owner_key_ids: sample_owner_key_ids
		};
		interactionTransform = next_transform;
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
		if (typeof window === 'undefined') {
			return;
		}
		const refresh_curve_theme = (): void => {
			const styles = window.getComputedStyle(document.documentElement);
			curve_canvas_theme = {
				canvas_bg: css_var_or(styles, '--gc-color-curve-canvas-bg', '#171b20'),
				canvas_wrap_bg: css_var_or(styles, '--gc-color-curve-canvas-wrap-bg', 'transparent'),
				canvas_wrap_border: css_var_or(
					styles,
					'--gc-color-curve-canvas-wrap-border',
					'transparent'
				),
				line_idle: css_var_or(styles, '--gc-color-curve-line-idle', '#9aa1ac'),
				point_idle: css_var_or(styles, '--gc-color-curve-point-idle', '#b1bac9'),
				fill_rgb: css_var_or(styles, '--gc-color-curve-fill-rgb', '80, 162, 255'),
				grid_line_rgb: css_var_or(styles, '--gc-color-curve-grid-line-rgb', '164, 174, 191'),
				grid_label_rgb: css_var_or(styles, '--gc-color-curve-grid-label-rgb', '186, 195, 210'),
				border_unbounded: css_var_or(
					styles,
					'--gc-color-curve-border-unbounded',
					'rgba(138, 147, 161, 0.46)'
				),
				border_bounded: css_var_or(
					styles,
					'--gc-color-curve-border-bounded',
					'rgba(185, 199, 219, 0.82)'
				),
				empty_text: css_var_or(styles, '--gc-color-curve-empty-text', 'rgba(192, 206, 232, 0.64)'),
				plot_fill: css_var_or(styles, '--gc-color-curve-plot-fill', 'rgba(0, 0, 0, 0.2)'),
				plot_shadow: css_var_or(styles, '--gc-color-curve-plot-shadow', 'rgba(0, 0, 0, 0.5)'),
				plot_shadow_fill: css_var_or(styles, '--gc-color-curve-plot-shadow-fill', '#000000'),
				hover_segment_underlay: css_var_or(
					styles,
					'--gc-color-curve-hover-segment-underlay',
					'rgba(92, 186, 255, 0.24)'
				),
				hover_segment: css_var_or(
					styles,
					'--gc-color-curve-hover-segment',
					'rgba(146, 214, 255, 0.74)'
				),
				selected_segment_underlay: css_var_or(
					styles,
					'--gc-color-curve-selected-segment-underlay',
					'rgba(92, 241, 170, 0.34)'
				),
				selected_segment: css_var_or(
					styles,
					'--gc-color-curve-selected-segment',
					'rgba(150, 255, 203, 0.96)'
				),
				handle_line: css_var_or(
					styles,
					'--gc-color-curve-handle-line',
					'rgba(222, 232, 251, 0.32)'
				),
				handle_line_aligned: css_var_or(
					styles,
					'--gc-color-curve-handle-line-aligned',
					'rgba(120, 234, 198, 0.46)'
				),
				handle_line_unlocked: css_var_or(
					styles,
					'--gc-color-curve-handle-line-unlocked',
					'rgba(255, 177, 118, 0.42)'
				),
				handle_active: css_var_or(
					styles,
					'--gc-color-curve-handle-active',
					'rgba(255, 224, 130, 0.98)'
				),
				handle_hover: css_var_or(
					styles,
					'--gc-color-curve-handle-hover',
					'rgba(255, 199, 138, 0.95)'
				),
				handle_out: css_var_or(styles, '--gc-color-curve-handle-out', 'rgba(172, 235, 220, 0.9)'),
				handle_in: css_var_or(styles, '--gc-color-curve-handle-in', 'rgba(164, 205, 245, 0.9)'),
				handle_stroke: css_var_or(
					styles,
					'--gc-color-curve-handle-stroke',
					'rgba(10, 16, 24, 0.95)'
				),
				handle_anchor_aligned: css_var_or(
					styles,
					'--gc-color-curve-handle-anchor-aligned',
					'rgba(141, 245, 205, 0.94)'
				),
				handle_anchor_unlocked: css_var_or(
					styles,
					'--gc-color-curve-handle-anchor-unlocked',
					'rgba(255, 183, 125, 0.94)'
				),
				key_selected: css_var_or(
					styles,
					'--gc-color-curve-key-selected',
					'rgba(255, 229, 112, 0.98)'
				),
				key_hover: css_var_or(styles, '--gc-color-curve-key-hover', 'rgba(255, 182, 96, 0.95)'),
				key_stroke: css_var_or(styles, '--gc-color-curve-key-stroke', 'rgba(8, 12, 18, 0.9)'),
				key_guide: css_var_or(styles, '--gc-color-curve-key-guide', 'rgba(255, 226, 117, 0.35)'),
				draw_path_stroke: css_var_or(
					styles,
					'--gc-color-curve-draw-path-stroke',
					'rgba(255, 140, 98, 0.96)'
				),
				draw_path_point: css_var_or(
					styles,
					'--gc-color-curve-draw-path-point',
					'rgba(255, 198, 122, 0.98)'
				),
				crosshair: css_var_or(styles, '--gc-color-curve-crosshair', 'rgba(217, 231, 255, 0)'),
				selection_fill: css_var_or(
					styles,
					'--gc-color-curve-selection-fill',
					'rgba(114, 210, 255, 0.14)'
				),
				selection_stroke: css_var_or(
					styles,
					'--gc-color-curve-selection-stroke',
					'rgba(158, 228, 255, 0.9)'
				),
				bounds_readout: css_var_or(
					styles,
					'--gc-color-curve-bounds-readout',
					'rgba(198, 208, 224, 0.86)'
				)
			};
		};
		refresh_curve_theme();

		if (typeof MutationObserver === 'undefined') {
			return;
		}
		const observer = new MutationObserver(refresh_curve_theme);
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style']
		});
		return () => {
			observer.disconnect();
		};
	});

	$effect(() => {
		if (!canvasElement || typeof window === 'undefined') {
			canvas_context = null;
			return;
		}
		const context = canvasElement.getContext('2d');
		if (!context) {
			canvas_context = null;
			return;
		}
		canvas_context = context;

		const sync_canvas_size = (): void => {
			if (!canvasElement) {
				return;
			}
			const rect = canvasElement.getBoundingClientRect();
			const css_width = Math.max(1, rect.width);
			const css_height = Math.max(1, rect.height);
			const dpr = Math.max(1, window.devicePixelRatio || 1);
			const device_width = Math.max(1, Math.round(css_width * dpr));
			const device_height = Math.max(1, Math.round(css_height * dpr));

			canvas_width_px = css_width;
			canvas_height_px = css_height;

			if (canvasElement.width !== device_width || canvasElement.height !== device_height) {
				canvasElement.width = device_width;
				canvasElement.height = device_height;
			}
			context.setTransform(dpr, 0, 0, dpr, 0, 0);
		};

		let resizeFrameId: number | null = null;
		const scheduleCanvasSizeSync = (): void => {
			if (typeof window === 'undefined') {
				sync_canvas_size();
				return;
			}
			if (resizeFrameId !== null) {
				return;
			}
			resizeFrameId = window.requestAnimationFrame(() => {
				resizeFrameId = null;
				sync_canvas_size();
			});
		};

		let observer: ResizeObserver | null = null;
		if (typeof ResizeObserver !== 'undefined') {
			observer = new ResizeObserver(() => {
				scheduleCanvasSizeSync();
			});
			observer.observe(canvasElement);
		}
		sync_canvas_size();

		return () => {
			if (resizeFrameId !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(resizeFrameId);
			}
			observer?.disconnect();
			canvas_context = null;
		};
	});

	$effect(() => {
		canvas_context;
		canvas_width_px;
		canvas_height_px;
		compiledCurve;
		parsedKeys;
		selectedKeyId;
		selectedKeyIds;
		selectedCurveOwnerKeyIds;
		hoverKeyId;
		hoverCurveOwnerKeyId;
		bezierHandles;
		activeBezierHandle;
		hoverBezierHandle;
		hoverCurvePosition;
		drawPathPoints;
		selectionRect;
		rem_base_px;
		curve_canvas_theme;
		fixedViewBounds;
		activeCurveRangeConstraint;
		showGrid;
		showNumbers;
		showBounds;
		draw_curve_canvas();
	});
</script>

<div class="curve-canvas-container" style:height={canvasHeight}>
	<div class="curve-canvas-wrap">
		<canvas
			class="curve-canvas {activeCanvasPan ? 'panning' : ''}"
			bind:this={canvasElement}
			data-local-context-menu
			tabindex="0"
			{onpointerdown}
			{onpointermove}
			{onpointerup}
			{onpointercancel}
			{onpointerleave}
			{onwheel}
			{oncontextmenu}
			{onfocus}
			{onblur}></canvas>
	</div>
	{#if showBounds}
		<div class="curve-bounds-readout">{bounds_label}</div>
	{/if}
</div>

<style>
	.curve-canvas-container {
		height: min(26rem, 38vh);
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 100%;
		min-width: 0;
		min-height: 0;
		gap: 0.18rem;
	}

	.curve-canvas-wrap {
		flex: 1 1 auto;
		width: 100%;
		max-width: 100%;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		background: var(--gc-color-curve-canvas-wrap-bg);
		border: solid 0.06rem var(--gc-color-curve-canvas-wrap-border);
		border-radius: 0.3rem;
	}

	.curve-canvas {
		display: block;
		width: 100%;
		max-width: 100%;
		height: 100%;
		cursor: crosshair;
		background: var(--gc-color-curve-canvas-bg);
	}

	.curve-canvas-wrap:focus,
	.curve-canvas:focus,
	.curve-canvas-wrap:focus-visible,
	.curve-canvas:focus-visible {
		outline: none;
	}

	.curve-canvas.panning {
		cursor: grabbing;
	}

	.curve-bounds-readout {
		font-size: 0.57rem;
		line-height: 1.25;
		padding: 0 0.16rem 0.1rem;
		color: var(--gc-color-curve-bounds-readout);
		opacity: 0.92;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	@media (max-width: 58rem) {
		.curve-canvas-container {
			height: min(22rem, 42vh);
		}
	}
</style>
