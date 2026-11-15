import { CURVE_EPSILON } from './constants';
import { clamp } from './values';
import type { NodeId } from '../../../types';
import type { BezierHandleRef, CurveSegmentHit } from './types';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorConstraints } from './editor-constraints';
import type { createCurveEditorModel } from './editor-model.svelte';
interface Context {
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly constraints: ReturnType<typeof createCurveEditorConstraints>;
	readonly model: ReturnType<typeof createCurveEditorModel>;
}
export function createCurveEditorHits(context: Context) {
	const canvas_local_point = (
		event: MouseEvent | PointerEvent
	): {
		x: number;
		y: number;
	} | null => {
		if (!context.state.canvas_element) {
			return null;
		}
		const rect = context.state.canvas_element.getBoundingClientRect();
		return {
			x: event.clientX - rect.left,
			y: event.clientY - rect.top
		};
	};
	const screen_to_curve_point = (
		screen_x: number,
		screen_y: number
	): {
		position: number;
		value: number;
	} | null => {
		if (!context.state.interaction_transform) {
			return null;
		}
		const normalized_x = clamp(
			(screen_x - context.state.interaction_transform.plot_left) /
				context.state.interaction_transform.plot_width,
			0,
			1
		);
		const normalized_y = clamp(
			(screen_y - context.state.interaction_transform.plot_top) /
				context.state.interaction_transform.plot_height,
			0,
			1
		);
		const curve_point = {
			position:
				context.state.interaction_transform.x_min +
				normalized_x *
					(context.state.interaction_transform.x_max - context.state.interaction_transform.x_min),
			value:
				context.state.interaction_transform.y_max -
				normalized_y *
					(context.state.interaction_transform.y_max - context.state.interaction_transform.y_min)
		};
		return context.constraints.clamp_curve_point_to_active_range(
			curve_point.position,
			curve_point.value
		);
	};
	const nearest_key = (screen_x: number, screen_y: number, threshold_px: number): NodeId | null => {
		if (!context.state.interaction_transform) {
			return null;
		}
		let result: NodeId | null = null;
		let best_distance = Number.POSITIVE_INFINITY;
		for (const [key_id, point] of context.state.interaction_transform.key_screen_points.entries()) {
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
		if (!context.state.interaction_transform) {
			return null;
		}
		const curve_x = context.state.interaction_transform.curve_screen_x;
		const curve_y = context.state.interaction_transform.curve_screen_y;
		const owners = context.state.interaction_transform.curve_owner_key_ids;
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
	const nearest_bezier_handle = (
		screen_x: number,
		screen_y: number,
		threshold_px: number
	): BezierHandleRef | null => {
		if (!context.state.interaction_transform || context.model.bezier_handle_controls.length === 0) {
			return null;
		}
		const transform = context.state.interaction_transform;
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
		for (const control of context.model.bezier_handle_controls) {
			consider(control.ref, control.handle_position, control.handle_value);
		}
		return nearest;
	};
	return {
		canvas_local_point,
		screen_to_curve_point,
		nearest_key,
		nearest_curve_hit,
		nearest_curve_owner_key,
		nearest_bezier_handle
	};
}
