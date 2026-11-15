import { clamp } from './values';
import { CURVE_EPSILON } from './constants';
import type { CurveCanvasTheme } from './canvas-theme';

export interface CurveCanvasStyle {
	readonly theme: CurveCanvasTheme;
	readonly rem: number;
}

interface GridBlendStep {
	step: number;
	weight: number;
}

const GRID_MANTISSAS = [1, 2, 5] as const;
const GRID_MAX_LINES = 4000;
const GRID_MIN_PIXEL_SPACING = 3.5;

export const formatCurveAxisValue = (value: number): string => {
	if (!Number.isFinite(value)) {
		return '0';
	}
	if (Math.abs(value) >= 1000 || (Math.abs(value) > 0 && Math.abs(value) < 0.001)) {
		return value.toExponential(2);
	}
	return value.toFixed(3).replace(/\.?0+$/, '');
};

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
	style: CurveCanvasStyle,
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

	context.strokeStyle = `rgba(${style.theme.grid_line_rgb}, ${alpha})`;
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

export const draw_axis_grid = (
	style: CurveCanvasStyle,
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
			style,
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
				style,
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
				style,
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

export const draw_axis_tick_labels = (
	style: CurveCanvasStyle,
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

			context.fillStyle = `rgba(${style.theme.grid_label_rgb}, ${label_alpha})`;
			let previous_right = Number.NEGATIVE_INFINITY;
			for (let index = start_index; index <= end_index; index += 1) {
				const value = index * step;
				const x = to_screen(value);
				if (x < plot_left || x > plot_left + plot_width) {
					continue;
				}
				const label = formatCurveAxisValue(value);
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

		context.fillStyle = `rgba(${style.theme.grid_label_rgb}, ${label_alpha})`;
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
			const label = formatCurveAxisValue(value);
			const text_width = context.measureText(label).width;
			context.fillText(label, label_right - text_width, y + safe_font * 0.31);
		}
	}
};

export const draw_plot_border = (
	style: CurveCanvasStyle,
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
	context.strokeStyle = bounded_range ? style.theme.border_bounded : style.theme.border_unbounded;
	context.lineWidth = bounded_range ? 1.15 : 0.95;
	if (bounded_range) {
		context.setLineDash([]);
	} else {
		const dash = Math.max(2, style.rem * 0.14);
		context.setLineDash([dash, dash * 0.9]);
	}
	context.beginPath();
	context.roundRect(border_x, border_y, border_width, border_height, border_radius);
	context.stroke();
	context.restore();
};
