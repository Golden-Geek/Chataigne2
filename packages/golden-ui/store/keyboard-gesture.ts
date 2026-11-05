export interface KeyboardOffset {
	x: number;
	y: number;
}
interface Client<T> {
	identity(target: T): string;
	start(target: T): boolean;
	move(pixels: KeyboardOffset): void;
	finish(): void;
	stop(): void;
}
type Key = Pick<KeyboardEvent, 'key' | 'shiftKey' | 'altKey' | 'ctrlKey' | 'metaKey' | 'repeat'>;
interface Gesture {
	identity: string;
	pressed: Set<string>;
	pixels: KeyboardOffset;
	closed: boolean;
}
const arrows: Record<string, KeyboardOffset> = {
	ArrowLeft: { x: -1, y: 0 },
	ArrowRight: { x: 1, y: 0 },
	ArrowUp: { x: 0, y: -1 },
	ArrowDown: { x: 0, y: 1 }
};
/** Bounded held-arrow input; callers own target identity, current geometry and edit admission. */
export class KeyboardGesture<T> {
	private owner: Gesture | null = null;
	private running: Gesture | null = null;
	private pending: Gesture[] = [];
	private ready = true;
	private client: Client<T>;
	constructor(client: Client<T>) {
		this.client = client;
	}
	keydown(event: Key, target: T | null): boolean {
		const delta = arrows[event.key];
		if (!delta || event.altKey || event.ctrlKey || event.metaKey || !target) return false;
		const identity = this.client.identity(target);
		if (this.owner && this.owner.identity !== identity) this.cancel();
		if (!this.owner) {
			// A held key from another document/selection must be released before starting here.
			if (event.repeat || this.pending.length >= 32) return false;
			this.owner = {
				identity,
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
	observe(target: T | null, owned: boolean, ready = true) {
		this.ready = ready;
		if (
			!target ||
			(this.running && this.running.identity !== this.client.identity(target)) ||
			this.pending.some((gesture) => gesture.identity !== this.client.identity(target))
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
	private drain(target: T): boolean {
		if (this.running || !this.ready || this.pending.length === 0) return true;
		const next = this.pending.shift()!;
		// A completed edit may have changed geometry. Begin from its current accepted projection,
		// never from the stale origin captured while the prior edit was closing.
		if (!this.client.start(target)) {
			this.cancel();
			return false;
		}
		this.running = next;
		this.client.move(next.pixels);
		if (next.closed) this.client.finish();
		return true;
	}
}
