import {
	getClosestSurfaceMetrics,
	getRootRemPixels,
	getViewportHeightPx,
	getViewportWidthPx
} from './dom';

/** A gesture's measured reference frame; checks happen on input/admission, never by polling. */
export function captureLayoutFrame(target: EventTarget | null, onInvalidated: () => void) {
	let surface =
		target instanceof Element ? target.closest<HTMLElement>('.dashboard-surface') : null;
	if (!surface?.isConnected) return null;
	const read = () => {
		if (!surface?.isConnected) return null;
		const rootRemPx = getRootRemPixels(),
			metrics = getClosestSurfaceMetrics(surface, rootRemPx);
		const rect = surface.getBoundingClientRect();
		return {
			metrics: {
				rootRemPx,
				emPx: metrics.emPx,
				surfaceWidthPx: metrics.width,
				surfaceHeightPx: metrics.height,
				viewportWidthPx: getViewportWidthPx(),
				viewportHeightPx: getViewportHeightPx(),
				surfaceScaleX: metrics.scaleX,
				surfaceScaleY: metrics.scaleY
			},
			frame: [rect.left, rect.top, rect.width, rect.height, surface.scrollLeft, surface.scrollTop],
			fonts: [rootRemPx, metrics.emPx]
		};
	};
	const before = read()!;
	let valid = true,
		disposed = false;
	const check = () => {
		if (!valid || disposed) return false;
		const next = read();
		valid =
			!!next &&
			before.frame.every((value, index) => Math.abs(value - next.frame[index]) <= 0.5) &&
			before.fonts.every((value, index) => Math.abs(value - next.fonts[index]) <= 0.001) &&
			Math.abs(before.metrics.surfaceWidthPx - next.metrics.surfaceWidthPx) <= 0.5 &&
			Math.abs(before.metrics.surfaceHeightPx - next.metrics.surfaceHeightPx) <= 0.5 &&
			before.metrics.viewportWidthPx === next.metrics.viewportWidthPx &&
			before.metrics.viewportHeightPx === next.metrics.viewportHeightPx;
		if (!valid)
			queueMicrotask(() => {
				if (!disposed) onInvalidated();
			});
		return valid;
	};
	window.addEventListener('resize', check);
	return {
		metrics: before.metrics,
		get current() {
			return valid && !disposed;
		},
		check,
		dispose() {
			disposed = true;
			surface = null;
			window.removeEventListener('resize', check);
		}
	};
}
export type LayoutFrame = NonNullable<ReturnType<typeof captureLayoutFrame>>;
