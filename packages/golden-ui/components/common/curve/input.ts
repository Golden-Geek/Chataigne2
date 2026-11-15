import type { NodeId } from '../../../types';
import type { UiEditSession } from '../../../store/ui-intents';
import { clearViewportSelection } from '../../viewport/selection';
import type {
	ActiveKeyDrag,
	ActiveBezierHandleDrag,
	ActiveCurveDrag,
	ActiveCanvasPan,
	ActiveCurveDraw,
	ActiveBoxSelection,
	DragPreview,
	BezierEasingPreview,
	RecordedCurveDrawPoint,
	CurveViewBounds
} from './types';

interface Context {
	readonly canvas: HTMLCanvasElement | null;
	key: ActiveKeyDrag | null;
	bezier: ActiveBezierHandleDrag | null;
	curve: ActiveCurveDrag | null;
	pan: ActiveCanvasPan | null;
	draw: ActiveCurveDraw | null;
	box: ActiveBoxSelection | null;
	readonly keyPreviews: Map<NodeId, DragPreview>;
	readonly easingPreviews: Map<NodeId, BezierEasingPreview>;
	flush(): void;
	discard(): void;
	takeEdit(): {
		session: UiEditSession;
		begin: Promise<void> | null;
		pending: Promise<void>[];
	} | null;
	clearPreviews(keys: Map<NodeId, DragPreview>, easing: Map<NodeId, BezierEasingPreview>): void;
	completeBox(box: ActiveBoxSelection): void;
	completeDraw(points: RecordedCurveDrawPoint[]): void;
	restoreView(bounds: CurveViewBounds): void;
	clearHover(): void;
}

/** Retire tool ownership before capture release. Cancellation never flushes unsent input. */
export function createCurveInput(context: Context) {
	const matches = (tool: { pointer_id: number } | null, pointer?: number) =>
		tool !== null && (pointer === undefined || tool.pointer_id === pointer);
	const release = (pointer: number) => {
		if (context.canvas?.hasPointerCapture(pointer)) context.canvas.releasePointerCapture(pointer);
	};
	function drag(pointer?: number, commit = true): boolean {
		const key = matches(context.key, pointer) ? context.key : null;
		const bezier = matches(context.bezier, pointer) ? context.bezier : null;
		const curve = matches(context.curve, pointer) ? context.curve : null;
		if (!key && !bezier && !curve) return false;
		const keys = new Map<NodeId, DragPreview>();
		const easing = new Map<NodeId, BezierEasingPreview>();
		const easingIds = new Set([
			...(key?.touched_bezier_key_ids ?? []),
			...(bezier?.touched_key_ids ?? []),
			...(curve?.touched_key_ids ?? [])
		]);
		for (const origin of key?.origins ?? []) {
			const preview = context.keyPreviews.get(origin.key_id);
			if (preview) keys.set(origin.key_id, preview);
		}
		for (const id of easingIds) {
			const preview = context.easingPreviews.get(id);
			if (preview) easing.set(id, preview);
		}
		if (key) context.key = null;
		if (bezier) context.bezier = null;
		if (curve) context.curve = null;
		if (commit) context.flush();
		else context.discard();
		const edit = context.takeEdit();
		for (const tool of [key, bezier, curve]) if (tool) release(tool.pointer_id);
		context.clearHover();
		if (!commit) context.clearPreviews(keys, easing);
		if (edit) {
			// Submitted writes keep their original history group; a later gesture owns new previews.
			void (async () => {
				if (edit.begin) await edit.begin;
				await Promise.allSettled(edit.pending);
				if (commit) context.clearPreviews(keys, easing);
				await edit.session.end();
			})();
		} else if (commit) context.clearPreviews(keys, easing);
		return true;
	}
	function pan(pointer?: number, commit = true): boolean {
		const active = context.pan;
		if (!matches(active, pointer) || !active) return false;
		context.pan = null;
		if (!commit) context.restoreView(active.start_bounds);
		release(active.pointer_id);
		return true;
	}
	function draw(pointer?: number, commit = true): boolean {
		const active = context.draw;
		if (!matches(active, pointer) || !active) return false;
		context.draw = null;
		release(active.pointer_id);
		if (commit) context.completeDraw(active.points);
		return true;
	}
	function box(pointer?: number, commit = true): boolean {
		const active = context.box;
		if (!matches(active, pointer) || !active) return false;
		context.box = null;
		release(active.pointer_id);
		if (commit) context.completeBox(active);
		return true;
	}
	return {
		get active() {
			return !!(
				context.key ||
				context.bezier ||
				context.curve ||
				context.pan ||
				context.draw ||
				context.box
			);
		},
		finish(pointer: number) {
			drag(pointer);
			pan(pointer);
			draw(pointer);
			box(pointer);
		},
		cancel(pointer?: number): boolean {
			// Evaluate every tool, even if an earlier tool owned input.
			const retired = [
				drag(pointer, false),
				pan(pointer, false),
				draw(pointer, false),
				box(pointer, false)
			];
			if (retired.some(Boolean)) context.clearHover();
			return retired.some(Boolean);
		}
	};
}

/** Canvas-local Escape and cancellation, with window loss and disposal as terminal boundaries. */
export function curveInputEvents(
	canvas: HTMLCanvasElement,
	options: { cancel(pointer?: number): boolean; clear(): void }
) {
	const key = (event: KeyboardEvent) => clearViewportSelection(event, options);
	const pointer = (event: PointerEvent) => options.cancel(event.pointerId);
	const blur = () => options.cancel();
	canvas.addEventListener('keydown', key);
	canvas.addEventListener('pointercancel', pointer);
	canvas.addEventListener('lostpointercapture', pointer);
	canvas.addEventListener('blur', blur);
	window.addEventListener('blur', blur);
	return () => {
		canvas.removeEventListener('keydown', key);
		canvas.removeEventListener('pointercancel', pointer);
		canvas.removeEventListener('lostpointercapture', pointer);
		canvas.removeEventListener('blur', blur);
		window.removeEventListener('blur', blur);
		options.cancel();
	};
}
