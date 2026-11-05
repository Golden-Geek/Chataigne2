import type { Point } from './generated/Point';
import type { PixelOffset } from './generated/PixelOffset';
import type { SurfaceReference } from './generated/SurfaceReference';

export interface KeyboardTarget {
	document: string;
	reference: SurfaceReference;
	corner: number;
	origin: Point;
}
interface Client {
	start(target: KeyboardTarget): boolean;
	move(pixels: PixelOffset): void;
	finish(): void;
	stop(): void;
}
type Key = Pick<KeyboardEvent, 'key' | 'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey' | 'repeat'>;
interface Gesture {
	target: KeyboardTarget;
	pressed: Set<string>;
	pixels: PixelOffset;
	closed: boolean;
}
const arrows: Record<string, PixelOffset> = {
	ArrowLeft: { x: -1, y: 0 },
	ArrowRight: { x: 1, y: 0 },
	ArrowUp: { x: 0, y: -1 },
	ArrowDown: { x: 0, y: 1 }
};
function same(a: KeyboardTarget, b: KeyboardTarget) {
	return (
		a.document === b.document &&
		a.corner === b.corner &&
		a.reference.screen === b.reference.screen &&
		a.reference.surface === b.reference.surface
	);
}

/** Collect integer key input; the backend owns output-pixel conversion and geometry validity. */
export class SurfaceKeyboard {
	private owner: Gesture | null = null;
	private running: Gesture | null = null;
	private pending: Gesture[] = [];
	private ready = true;
	private client: Client;
	constructor(client: Client) {
		this.client = client;
	}
	keydown(event: Key, target: KeyboardTarget | null): boolean {
		const delta = arrows[event.key];
		if (!delta || event.altKey || event.ctrlKey || event.metaKey || !target) return false;
		if (this.owner && !same(this.owner.target, target)) this.cancel();
		if (!this.owner) {
			// A held key from another document/selection must be released before starting here.
			if (event.repeat || this.pending.length >= 32) return false;
			this.owner = {
				target: { ...target, reference: { ...target.reference }, origin: { ...target.origin } },
				pressed: new Set(),
				pixels: { x: 0, y: 0 },
				closed: false
			};
			this.pending.push(this.owner);
		}
		const owner = this.owner;
		owner.pressed.add(event.key);
		const step = event.shiftKey ? 10 : 1;
		owner.pixels = { x: owner.pixels.x + step * delta.x, y: owner.pixels.y + step * delta.y };
		if (owner === this.running) this.client.move(owner.pixels);
		else if (!this.drain(target)) return false;
		return true;
	}
	keyup(key: string) {
		const owner = this.owner;
		if (owner?.pressed.delete(key) && owner.pressed.size === 0) {
			owner.closed = true;
			this.owner = null;
			if (owner === this.running) this.client.finish();
		}
	}
	observe(target: KeyboardTarget | null, owned: boolean, ready = true) {
		this.ready = ready;
		if (
			!target ||
			(this.running && !same(this.running.target, target)) ||
			this.pending.some((gesture) => !same(gesture.target, target))
		) {
			this.cancel();
			return;
		}
		if (this.running && !owned) {
			if (!this.running.closed) {
				this.cancel();
				return;
			}
			this.running = null;
		}
		this.drain(target);
	}
	finish() {
		// Focus loss flushes the active gesture, but must not replay queued input later.
		this.pending = [];
		this.owner = null;
		if (this.running && !this.running.closed) {
			this.running.closed = true;
			this.client.finish();
		}
	}
	cancel() {
		this.pending = [];
		this.owner = null;
		if (this.running) this.client.stop();
		this.running = null;
	}
	private drain(target: KeyboardTarget): boolean {
		if (this.running || !this.ready || this.pending.length === 0) return true;
		const next = this.pending.shift()!;
		// A completed edit may have changed the corner. Begin from its current accepted projection,
		// never from the stale origin captured while the prior edit was closing.
		if (!this.client.start({ ...target, origin: { ...target.origin } })) {
			this.cancel();
			return false;
		}
		this.running = next;
		this.client.move(next.pixels);
		if (next.closed) this.client.finish();
		return true;
	}
}
