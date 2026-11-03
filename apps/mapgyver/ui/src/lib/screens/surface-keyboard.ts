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
	private owner: KeyboardTarget | null = null;
	private pressed = new Set<string>();
	private pixels: PixelOffset = { x: 0, y: 0 };
	private client: Client;
	constructor(client: Client) {
		this.client = client;
	}
	keydown(event: Key, target: KeyboardTarget | null): boolean {
		const delta = arrows[event.key];
		if (!delta || event.altKey || event.ctrlKey || event.metaKey || !target) return false;
		if (this.owner && !same(this.owner, target)) this.cancel();
		if (!this.owner) {
			// A held key from another document/selection must be released before starting here.
			if (event.repeat || !this.client.start(target)) return false;
			this.owner = { ...target, reference: { ...target.reference }, origin: { ...target.origin } };
			this.pixels = { x: 0, y: 0 };
		}
		this.pressed.add(event.key);
		const step = event.shiftKey ? 10 : 1;
		this.pixels = { x: this.pixels.x + step * delta.x, y: this.pixels.y + step * delta.y };
		this.client.move(this.pixels);
		return true;
	}
	keyup(key: string) {
		if (this.pressed.delete(key) && this.pressed.size === 0) this.finish();
	}
	observe(target: KeyboardTarget | null, owned: boolean) {
		if (this.owner && (!owned || !target || !same(this.owner, target))) this.cancel();
	}
	finish() {
		this.end(false);
	}
	cancel() {
		this.end(true);
	}
	private end(cancel: boolean) {
		if (!this.owner) return;
		this.owner = null;
		this.pressed.clear();
		if (cancel) this.client.stop();
		else this.client.finish();
	}
}
