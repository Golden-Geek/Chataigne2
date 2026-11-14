import type { ViewPoint } from './geometry';

interface Client {
	canvas(): Element | undefined;
	extent(): { width: number; height: number };
	ghost(point: ViewPoint | null): void;
	move(point: ViewPoint): void;
	finish(): void;
	stop(): void;
}

/** Select a primary hit before edit admission; adapters own target identities and mutations. */
export function createViewportPointer(client: Client) {
	let pointer: number | null = null;
	function point(event: PointerEvent): ViewPoint | null {
		const rect = client.canvas()?.getBoundingClientRect();
		if (!rect || rect.width <= 0 || rect.height <= 0) return null;
		const extent = client.extent();
		if (!(extent.width > 0 && extent.height > 0 && Number.isFinite(extent.width + extent.height)))
			return null;
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
		start(event: PointerEvent, select: () => void, begin: (point: ViewPoint) => boolean) {
			const at = point(event),
				canvas = client.canvas();
			if (!canvas || event.defaultPrevented || event.button !== 0 || pointer !== null || !at)
				return false;
			event.stopPropagation();
			// Keep the viewport's focus when selection replaces a hit control, even if editing is refused.
			event.preventDefault();
			select();
			if (!begin(at)) return false;
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
