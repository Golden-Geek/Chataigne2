import {
	CURVE_EPSILON,
	MIN_VIEW_SPAN,
	CURVE_DRAW_FIT_MAX_ERROR_PX,
	CURVE_DRAW_FIT_MAX_KEYS
} from './constants';
import { clamp } from './values';
import type { RecordedCurveDrawPoint, ActiveCurveDraw } from './types';
import type { CurveEditorServices } from './editor-context';
import type { createCurveEditorState } from './editor-state.svelte';
import type { createCurveEditorModel } from './editor-model.svelte';
import type { createCurveEditorCamera } from './editor-camera';
interface Context {
	readonly state: ReturnType<typeof createCurveEditorState>;
	readonly model: ReturnType<typeof createCurveEditorModel>;
	readonly camera: ReturnType<typeof createCurveEditorCamera>;
	readonly services: CurveEditorServices;
}
export function createCurveEditorDrawing(context: Context) {
	const curve_draw_bucket = (screen_x: number): number | null => {
		if (!context.state.interaction_transform) {
			return null;
		}
		return Math.round(
			clamp(
				screen_x - context.state.interaction_transform.plot_left,
				0,
				context.state.interaction_transform.plot_width
			)
		);
	};
	const record_curve_draw_point = (
		active: ActiveCurveDraw,
		screen_x: number,
		curve_point: {
			position: number;
			value: number;
		}
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
		if (context.state.interaction_transform) {
			const value_span =
				context.state.interaction_transform.y_max - context.state.interaction_transform.y_min;
			if (Number.isFinite(value_span) && value_span > CURVE_EPSILON) {
				return Math.max(
					MIN_VIEW_SPAN,
					(value_span / Math.max(1, context.state.interaction_transform.plot_height)) *
						CURVE_DRAW_FIT_MAX_ERROR_PX
				);
			}
		}
		const active_range = context.model.active_curve_range_constraint;
		if (active_range) {
			return Math.max(MIN_VIEW_SPAN, (active_range.y_max - active_range.y_min) * 0.01);
		}
		const current = context.camera.current_view_bounds();
		if (current) {
			return Math.max(MIN_VIEW_SPAN, (current.y_max - current.y_min) * 0.01);
		}
		return 0.01;
	};
	const commit_curve_draw = async (points: RecordedCurveDrawPoint[]): Promise<void> => {
		await context.services.curveOperations.draw(
			points.map(({ position, value }) => ({ position, value })),
			{
				max_value_error: curve_draw_fit_value_error(),
				max_keys: CURVE_DRAW_FIT_MAX_KEYS
			}
		);
	};
	return {
		curve_draw_bucket,
		record_curve_draw_point,
		curve_draw_fit_value_error,
		commit_curve_draw
	};
}
