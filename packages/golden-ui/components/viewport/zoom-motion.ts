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

/** One bounded animation; retarget from the displayed camera while accumulating requested zoom. */
export function createZoomMotion(
	read: () => ViewCamera,
	write: (camera: ViewCamera) => void,
	clock: AnimationClock = browserClock
) {
	let frame: number | null = null;
	let motion: { from: ViewCamera; to: ViewCamera; start: number } | null = null;
	let disposed = false;
	function cancel() {
		if (frame !== null) clock.cancel(frame);
		frame = null;
		motion = null;
	}
	function advance(time: number) {
		frame = null;
		if (!motion || disposed) return;
		const { from, to, start } = motion;
		const progress = Math.min(1, Math.max(0, (time - start) / DURATION_MS));
		if (progress === 1) {
			motion = null;
			write(to);
			return;
		}
		const eased = 1 - (1 - progress) ** 3;
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
	return {
		zoom(factor: number, anchor: ViewPoint, immediate = false) {
			if (
				disposed ||
				!Number.isFinite(factor) ||
				factor <= 0 ||
				!Number.isFinite(anchor.x + anchor.y)
			)
				return;
			const from = read();
			const to = zoomAt(from, (motion?.to.zoom ?? from.zoom) * factor, anchor);
			cancel();
			if (immediate) {
				write(to);
				return;
			}
			if (from.zoom === to.zoom && from.pan.x === to.pan.x && from.pan.y === to.pan.y) return;
			motion = { from, to, start: clock.now() };
			frame = clock.request(advance);
		},
		cancel,
		dispose() {
			disposed = true;
			cancel();
		}
	};
}
