import { zoomAt, type ViewCamera, type ViewPoint } from './geometry';

interface AnimationClock {
	now(): number;
	request(callback: (time: number) => void): number;
	cancel(frame: number): void;
}
const browserClock: AnimationClock = {
	now: () => performance.now(),
	request: (callback) => requestAnimationFrame(callback),
	cancel: (frame) => cancelAnimationFrame(frame)
};
const DURATION_MS = 160;

/** One displayed camera for zoom and eased navigation; pointer editing can freeze it immediately. */
export function createCameraMotion(
	read: () => ViewCamera,
	write: (camera: ViewCamera) => void,
	clock: AnimationClock = browserClock
) {
	let frame: number | null = null;
	let motion: {
		from: ViewCamera;
		to: ViewCamera;
		start: number;
		kind: 'zoom' | 'navigation';
	} | null = null;
	let disposed = false;
	function cancel() {
		if (frame !== null) clock.cancel(frame);
		frame = null;
		motion = null;
	}
	function advance(time: number) {
		frame = null;
		if (!motion || disposed) return;
		const { from, to, start, kind } = motion;
		const progress = Math.min(1, Math.max(0, (time - start) / DURATION_MS));
		if (progress === 1) {
			motion = null;
			write(to);
			return;
		}
		const eased =
			kind === 'navigation' ? progress * progress * (3 - 2 * progress) : 1 - (1 - progress) ** 3;
		// Linear camera interpolation keeps the chosen world point fixed at the anchor
		// at every intermediate scale, rather than easing scale and pan independently.
		write({
			zoom: from.zoom + (to.zoom - from.zoom) * eased,
			pan: {
				x: from.pan.x + (to.pan.x - from.pan.x) * eased,
				y: from.pan.y + (to.pan.y - from.pan.y) * eased
			}
		});
		frame = clock.request(advance);
	}
	function same(a: ViewCamera, b: ViewCamera) {
		return a.zoom === b.zoom && a.pan.x === b.pan.x && a.pan.y === b.pan.y;
	}
	function start(to: ViewCamera, kind: 'zoom' | 'navigation', immediate: boolean) {
		if (disposed || ![to.zoom, to.pan.x, to.pan.y].every(Number.isFinite) || to.zoom <= 0) return;
		// Held shortcuts must not keep resetting an identical destination's deadline.
		if (!immediate && motion && same(motion.to, to) && motion.kind === kind) return;
		const from = read();
		cancel();
		if (immediate) {
			write(to);
			return;
		}
		if (same(from, to)) return;
		motion = { from, to, start: clock.now(), kind };
		frame = clock.request(advance);
	}
	return {
		move(to: ViewCamera, immediate = false) {
			start({ zoom: to.zoom, pan: { ...to.pan } }, 'navigation', immediate);
		},
		zoom(factor: number, anchor: ViewPoint, immediate = false) {
			if (
				disposed ||
				!Number.isFinite(factor) ||
				factor <= 0 ||
				!Number.isFinite(anchor.x + anchor.y)
			)
				return;
			const from = read();
			const requested = motion?.kind === 'zoom' ? motion.to.zoom : from.zoom;
			start(zoomAt(from, requested * factor, anchor), 'zoom', immediate);
		},
		cancel,
		dispose() {
			disposed = true;
			cancel();
		}
	};
}
