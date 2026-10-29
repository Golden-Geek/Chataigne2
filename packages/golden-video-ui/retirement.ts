import type { CssRect } from './generated/CssRect';

const union = (a: CssRect, b: CssRect): CssRect => {
	const x = Math.min(a.x, b.x),
		y = Math.min(a.y, b.y);
	return {
		x,
		y,
		width: Math.max(a.x + a.width, b.x + b.width) - x,
		height: Math.max(a.y + a.height, b.y + b.height) - y
	};
};

/** CSS retirement masks survive component removal. Churn while IPC is blocked cannot allocate
 * one mask per historical view forever: overflow merges conservatively into an existing mask. */
export class RetirementCovers {
	private readonly covers = new Map<string, CssRect>();
	constructor(private readonly maximum = 64) {
		if (!Number.isSafeInteger(maximum) || maximum < 1) throw new Error('Invalid cover capacity');
	}
	protect(id: string, rect: CssRect) {
		if (
			![rect.x, rect.y, rect.width, rect.height].every(Number.isFinite) ||
			rect.width <= 0 ||
			rect.height <= 0
		)
			return;
		const key =
			this.covers.has(id) || this.covers.size < this.maximum
				? id
				: this.covers.keys().next().value!;
		const previous = this.covers.get(key);
		this.covers.set(key, previous ? union(previous, rect) : { ...rect });
	}
	get size() {
		return this.covers.size;
	}
	entries(): ReadonlyArray<readonly [string, CssRect]> {
		return [...this.covers];
	}
	clear() {
		this.covers.clear();
	}
}
