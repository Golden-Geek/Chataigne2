import type { Point } from '../media/generated/Point';

interface Client {
	canvas(): SVGSVGElement | undefined;
	extent(): { width: number; height: number };
	ghost(point: Point | null): void;
	move(point: Point): void;
	finish(): void;
	stop(): void;
}

/** Shared native-canvas pointer capture; adapters own typed targets and backend intents. */
export function createSpatialPointer(client: Client) {
	let pointer: number | null = null;
	function point(event: PointerEvent): Point | null {
		const rect = client.canvas()?.getBoundingClientRect();
		if (!rect || rect.width <= 0 || rect.height <= 0) return null;
		const extent = client.extent();
		return {
			x: ((event.clientX - rect.left) / rect.width) * extent.width,
			y: ((event.clientY - rect.top) / rect.height) * extent.height
		};
	}
	function cancel() {
		if (pointer === null) return;
		const previous = pointer;
		pointer = null;
		client.ghost(null);
		const canvas = client.canvas();
		if (canvas?.hasPointerCapture(previous)) canvas.releasePointerCapture(previous);
		client.stop();
	}
	return {
		get active() {
			return pointer !== null;
		},
		start(event: PointerEvent, begin: (point: Point) => boolean) {
			const at = point(event),
				canvas = client.canvas();
			if (!canvas || event.button !== 0 || pointer !== null || !at || !begin(at)) return false;
			event.preventDefault();
			event.stopPropagation();
			pointer = event.pointerId;
			client.ghost(at);
			canvas.setPointerCapture(pointer);
			return true;
		},
		move(event: PointerEvent) {
			if (event.pointerId !== pointer) return;
			const at = point(event);
			client.ghost(at);
			if (at) client.move(at);
			else cancel();
		},
		release(event: PointerEvent, cancelled = false) {
			if (event.pointerId !== pointer) return;
			if (cancelled) {
				cancel();
				return;
			}
			const at = point(event);
			if (at) client.move(at);
			client.finish();
			const previous = pointer;
			pointer = null;
			client.ghost(null);
			const canvas = client.canvas();
			if (previous !== null && canvas?.hasPointerCapture(previous))
				canvas.releasePointerCapture(previous);
		},
		cancel
	};
}
