import type { CssRect } from './generated/CssRect';
import { intersectRect } from './geometry';

export const MAX_RETIREMENT_COVERS = 64;
export const MAX_RETIREMENT_MASKS = MAX_RETIREMENT_COVERS * 4;

/** Cover stale/newly exposed pixels without covering the still-presented overlap. */
export function subtractRect(rect: CssRect, aperture?: CssRect): CssRect[] {
	if (!aperture) return [rect];
	const overlap = intersectRect(rect, aperture);
	if (overlap.width <= 0 || overlap.height <= 0) return [rect];
	return [
		{ x: rect.x, y: rect.y, width: rect.width, height: overlap.y - rect.y },
		{
			x: rect.x,
			y: overlap.y + overlap.height,
			width: rect.width,
			height: rect.y + rect.height - overlap.y - overlap.height
		},
		{ x: rect.x, y: overlap.y, width: overlap.x - rect.x, height: overlap.height },
		{
			x: overlap.x + overlap.width,
			y: overlap.y,
			width: rect.x + rect.width - overlap.x - overlap.width,
			height: overlap.height
		}
	].filter((piece) => piece.width > 0 && piece.height > 0);
}

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
	constructor(private readonly maximum = MAX_RETIREMENT_COVERS) {
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
