import type { NodeId } from '../../../types';
import type {
	ParsedCurveKey,
	CompiledCurve,
	CanvasTransform,
	CurveViewBounds,
	CurveRangeConstraint,
	CanvasSelectionRect,
	BezierHandleVisual,
	BezierHandleRef
} from './types';
import { clamp } from './values';
import { CURVE_EPSILON } from './constants';
import { buildCurveSamples } from './canvas-samples';
import {
	draw_axis_grid,
	draw_axis_tick_labels,
	draw_plot_border,
	formatCurveAxisValue,
	type CurveCanvasStyle
} from './canvas-grid';

const CURVE_BOUNDS_SAMPLE_MIN = 192;
const CURVE_BOUNDS_SAMPLE_MAX = 2048;
const CURVE_DRAW_MIN_DELTA_PX = 0.08;

export interface CurveCanvasFrame extends CurveCanvasStyle {
	readonly width: number;
	readonly height: number;
	readonly parsedKeys: readonly ParsedCurveKey[];
	readonly compiledCurve: CompiledCurve;
	readonly selectedKeyId: NodeId | null;
	readonly selectedKeyIds: readonly NodeId[];
	readonly selectedCurveOwnerKeyIds: readonly NodeId[];
	readonly hoverKeyId: NodeId | null;
	readonly hoverCurveOwnerKeyId: NodeId | null;
	readonly bezierHandles: readonly BezierHandleVisual[];
	readonly activeBezierHandle: BezierHandleRef | null;
	readonly hoverBezierHandle: BezierHandleRef | null;
	readonly hoverCurvePosition: Pick<ParsedCurveKey, 'position' | 'value'> | null;
	readonly drawPathPoints: readonly Pick<ParsedCurveKey, 'position' | 'value'>[];
	readonly selectionRect: CanvasSelectionRect | null;
	readonly activeCurveRangeConstraint: CurveRangeConstraint | null;
	readonly fixedViewBounds: CurveViewBounds | null;
	readonly showGrid: boolean;
	readonly showNumbers: boolean;
	sampleCurveAt(position: number): number | null;
}
export interface CurveCanvasResult {
	readonly transform: CanvasTransform | null;
	readonly boundsLabel: string;
}

export function paintCurveCanvas(
	context: CanvasRenderingContext2D,
	frame: CurveCanvasFrame
): CurveCanvasResult {
	const {
		theme: curve_canvas_theme,
		rem: rem_base_px,
		parsedKeys,
		compiledCurve,
		selectedKeyId,
		selectedKeyIds,
		selectedCurveOwnerKeyIds,
		hoverKeyId,
		hoverCurveOwnerKeyId,
		bezierHandles,
		activeBezierHandle,
		hoverBezierHandle,
		hoverCurvePosition,
		drawPathPoints,
		selectionRect,
		activeCurveRangeConstraint,
		fixedViewBounds,
		showGrid,
		showNumbers,
		sampleCurveAt
	} = frame;
	const width = Math.max(1, frame.width),
		height = Math.max(1, frame.height);
	context.clearRect(0, 0, width, height);

	const keys = compiledCurve.keys;
	if (keys.length === 0) {
		context.fillStyle = curve_canvas_theme.empty_text;
		context.font = `${Math.max(9, Math.round(height * 0.065))}px sans-serif`;
		context.fillText('No keys in this curve', width * 0.04, height * 0.18);

		const empty_bounds_label = 'Bounds : X [min: -, max: -] | Y [min: -, max: -]';
		return { transform: null, boundsLabel: empty_bounds_label };
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
		const position = index + 1 === bounds_sample_count ? x_max : x_min + bounds_sample_step * index;
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

	const sampled_curve = buildCurveSamples(
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
			frame,
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
			frame,
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
			frame,
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
			frame,
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
			frame,
			context,
			plot_left,
			plot_top,
			plot_width,
			plot_height,
			border_radius,
			Boolean(active_range)
		);
	}

	const next_bounds_label = `Bounds : X [min: ${formatCurveAxisValue(x_min)}, max: ${formatCurveAxisValue(x_max)}] | Y [min: ${formatCurveAxisValue(y_min)}, max: ${formatCurveAxisValue(y_max)}]`;

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
	return { transform: next_transform, boundsLabel: next_bounds_label };
}
