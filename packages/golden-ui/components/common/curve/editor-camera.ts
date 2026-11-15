import {
	CURVE_EPSILON,
	MIN_VIEW_SPAN,
	MAX_VIEW_SPAN,
	VIEW_BOUNDS_ANIMATION_DURATION_MS
} from './constants';
import { clamp } from './values';
import { sample_compiled_curve } from './evaluation';
import type { ParsedCurveKey, CurveViewBounds } from './types';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorHits } from './editor-hits';
import type { createCurveEditorSelection } from './editor-selection.svelte';
interface Context {
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly hits: ReturnType<typeof createCurveEditorHits>;
	readonly selection: ReturnType<typeof createCurveEditorSelection>;
}
export function createCurveEditorCamera(context: Context) {
	const current_view_bounds = (): CurveViewBounds | null => {
		if (context.state.fixed_view_bounds) {
			return { ...context.state.fixed_view_bounds };
		}
		if (!context.state.interaction_transform) {
			return null;
		}
		return {
			x_min: context.state.interaction_transform.x_min,
			x_max: context.state.interaction_transform.x_max,
			y_min: context.state.interaction_transform.y_min,
			y_max: context.state.interaction_transform.y_max
		};
	};
	const padded_axis_bounds = (
		min_candidate: number,
		max_candidate: number,
		minimum_span_candidate: number,
		pad_ratio: number
	): {
		min: number;
		max: number;
	} | null => {
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
	const estimate_frame_x_span = (keys: ParsedCurveKey[]): number => {
		if (keys.length >= 2) {
			return Math.max(MIN_VIEW_SPAN, keys[keys.length - 1].position - keys[0].position);
		}
		const focus = keys[0];
		if (!focus) {
			const active_range = context.model.active_curve_range_constraint;
			if (active_range) {
				return Math.max(MIN_VIEW_SPAN, active_range.x_max - active_range.x_min);
			}
			const current = current_view_bounds();
			if (current) {
				return Math.max(MIN_VIEW_SPAN, current.x_max - current.x_min);
			}
			return 1;
		}
		const index = context.model.parsed_keys.findIndex((entry) => entry.node_id === focus.node_id);
		const previous = index > 0 ? context.model.parsed_keys[index - 1] : null;
		const next =
			index >= 0 && index + 1 < context.model.parsed_keys.length
				? context.model.parsed_keys[index + 1]
				: null;
		if (previous && next) {
			return Math.max(MIN_VIEW_SPAN, next.position - previous.position);
		}
		if (previous) {
			return Math.max(MIN_VIEW_SPAN, (focus.position - previous.position) * 2);
		}
		if (next) {
			return Math.max(MIN_VIEW_SPAN, (next.position - focus.position) * 2);
		}
		const active_range = context.model.active_curve_range_constraint;
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
	): {
		min: number;
		max: number;
	} | null => {
		if (
			context.model.compiled_curve.keys.length === 0 ||
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
			const sampled = sample_compiled_curve(context.model.compiled_curve, position);
			if (sampled === null) {
				continue;
			}
			const value = context.constraints.clamp_curve_value_to_active_range(sampled);
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
		const active_range = context.model.active_curve_range_constraint;
		if (active_range) {
			return {
				x_min: active_range.x_min,
				x_max: active_range.x_max,
				y_min: active_range.y_min,
				y_max: active_range.y_max
			};
		}
		if (context.model.compiled_curve.keys.length === 0) {
			return null;
		}
		let x_min = context.model.compiled_curve.keys[0]?.position ?? 0;
		let x_max =
			context.model.compiled_curve.keys[context.model.compiled_curve.keys.length - 1]?.position ??
			x_min;
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
		for (const key of context.model.parsed_keys) {
			const value = context.constraints.clamp_curve_value_to_active_range(key.value);
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
	const sanitize_view_bounds = (bounds: CurveViewBounds): CurveViewBounds | null => {
		if (
			!Number.isFinite(bounds.x_min) ||
			!Number.isFinite(bounds.x_max) ||
			!Number.isFinite(bounds.y_min) ||
			!Number.isFinite(bounds.y_max)
		) {
			return null;
		}
		const active_range = context.model.active_curve_range_constraint;
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
		if (context.state.view_bounds_animation_frame !== 0 && typeof window !== 'undefined') {
			window.cancelAnimationFrame(context.state.view_bounds_animation_frame);
		}
		context.state.view_bounds_animation_frame = 0;
		context.state.view_bounds_animation_target = null;
	};
	const set_view_bounds_instantly = (candidate: CurveViewBounds): void => {
		const sanitized = sanitize_view_bounds({ ...candidate });
		if (!sanitized) {
			return;
		}
		cancel_view_bounds_animation();
		context.state.fixed_view_bounds = sanitized;
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
		context.state.fixed_view_bounds = start_bounds;
		context.state.view_bounds_animation_target = target_bounds;
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
				context.state.fixed_view_bounds = interpolated_bounds;
			}
			if (progress >= 1) {
				context.state.view_bounds_animation_frame = 0;
				context.state.view_bounds_animation_target = null;
				context.state.fixed_view_bounds = target_bounds;
				return;
			}
			context.state.view_bounds_animation_frame = window.requestAnimationFrame(step);
		};
		context.state.view_bounds_animation_frame = window.requestAnimationFrame(step);
	};
	const apply_view_bounds = (
		candidate: CurveViewBounds,
		options: {
			animate?: boolean;
		} = {}
	): void => {
		if (options.animate) {
			animate_view_bounds_to(candidate);
			return;
		}
		set_view_bounds_instantly(candidate);
	};
	const pan_view_by_pixels = (delta_screen_x: number, delta_screen_y: number): void => {
		const base = current_view_bounds();
		if (!base || !context.state.interaction_transform) {
			return;
		}
		const units_per_px_x =
			(base.x_max - base.x_min) / Math.max(1, context.state.interaction_transform.plot_width);
		const units_per_px_y =
			(base.y_max - base.y_min) / Math.max(1, context.state.interaction_transform.plot_height);
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
		if (!context.state.interaction_transform) {
			return;
		}
		const anchor = context.hits.screen_to_curve_point(screen_x, screen_y);
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
	const normalize_wheel_delta = (
		event: WheelEvent
	): {
		x: number;
		y: number;
	} => {
		let scale = 1;
		if (event.deltaMode === 1) {
			scale = context.state.rem_base_px * 1.3;
		} else if (event.deltaMode === 2) {
			const rect = context.state.canvas_element?.getBoundingClientRect();
			const width = rect ? Math.max(1, rect.width) : 1;
			const height = rect ? Math.max(1, rect.height) : 1;
			scale = Math.max(width, height);
		}
		return {
			x: event.deltaX * scale,
			y: event.deltaY * scale
		};
	};
	const frame_selected_view = (): boolean => {
		const frame_keys = context.selection
			.selected_frame_key_ids()
			.map((key_id) => context.model.parsed_key_by_id.get(key_id))
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
			const value = context.constraints.clamp_curve_value_to_active_range(key.value);
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
	const axis_midpoint = (): {
		position: number;
		value: number;
	} => {
		if (!context.state.interaction_transform) {
			if (context.model.active_curve_range_constraint) {
				return {
					position:
						(context.model.active_curve_range_constraint.x_min +
							context.model.active_curve_range_constraint.x_max) *
						0.5,
					value:
						(context.model.active_curve_range_constraint.y_min +
							context.model.active_curve_range_constraint.y_max) *
						0.5
				};
			}
			return { position: 0, value: 0 };
		}
		return {
			position:
				(context.state.interaction_transform.x_min + context.state.interaction_transform.x_max) *
				0.5,
			value:
				(context.state.interaction_transform.y_min + context.state.interaction_transform.y_max) *
				0.5
		};
	};
	return {
		current_view_bounds,
		padded_axis_bounds,
		estimate_frame_x_span,
		sampled_curve_value_bounds,
		default_curve_view_bounds,
		sanitize_view_bounds,
		same_view_bounds,
		cancel_view_bounds_animation,
		set_view_bounds_instantly,
		animate_view_bounds_to,
		apply_view_bounds,
		pan_view_by_pixels,
		zoom_view_at_screen,
		normalize_wheel_delta,
		frame_selected_view,
		home_view,
		axis_midpoint
	};
}
