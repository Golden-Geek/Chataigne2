import type { ViewBounds, ViewPoint } from './geometry';

export interface AlignmentGuide {
	axis: 'x' | 'y';
	value: number;
}
type Projection = (point: ViewPoint) => ViewBounds | null;
type Match = AlignmentGuide & {
	feature: number;
	a: number;
	b: number;
	distance: number;
	delta: number;
};
const anchors = (b: ViewBounds, axis: 'x' | 'y') => {
	const size = axis === 'x' ? b.width : b.height;
	return [b[axis], b[axis] + size / 2, b[axis] + size];
};
const valid = (b: ViewBounds | null): b is ViewBounds =>
	!!b && [b.x, b.y, b.width, b.height].every(Number.isFinite) && b.width >= 0 && b.height >= 0;
function nearest(sorted: number[], value: number) {
	let lo = 0,
		hi = sorted.length;
	while (lo < hi) {
		const mid = (lo + hi) >>> 1;
		if (sorted[mid] < value) lo = mid + 1;
		else hi = mid;
	}
	return [sorted[lo - 1], sorted[lo]]
		.filter(Number.isFinite)
		.sort((a, b) => Math.abs(a - value) - Math.abs(b - value))[0];
}

/** Viewport hints only. Index candidates once per gesture; the caller owns typed mutations.
 * The projection is locally affine for translation/scale, including rotated rectangles.
 * Fixed resize edges have zero derivative and cannot attract or fabricate a guide.
 */
export class ViewportAlignment {
	private coordinates: Record<'x' | 'y', number[]>;
	private locks: Partial<Record<'x' | 'y', Match>> = {};
	private features: readonly number[];
	constructor(
		bounds: readonly ViewBounds[],
		private project: Projection,
		options: { centers?: boolean } = {}
	) {
		this.features = options.centers === false ? [0, 2] : [0, 1, 2];
		const usable = bounds.filter((b) => valid(b) && b.width > 0 && b.height > 0);
		this.coordinates = { x: [], y: [] };
		for (const axis of ['x', 'y'] as const)
			this.coordinates[axis] = [...new Set(usable.flatMap((b) => anchors(b, axis)))].sort(
				(a, b) => a - b
			);
	}
	clear() {
		this.locks = {};
	}
	snap(point: ViewPoint, tolerance: ViewPoint): { point: ViewPoint; guides: AlignmentGuide[] } {
		const none = () => {
			this.clear();
			return { point, guides: [] };
		};
		if (
			![point.x, point.y, tolerance.x, tolerance.y].every(Number.isFinite) ||
			tolerance.x <= 0 ||
			tolerance.y <= 0
		)
			return none();
		const stepX = Math.max(1e-7, tolerance.x * 0.001),
			stepY = Math.max(1e-7, tolerance.y * 0.001);
		const base = this.project(point),
			dx = this.project({ x: point.x + stepX, y: point.y }),
			dy = this.project({ x: point.x, y: point.y + stepY });
		if (!valid(base) || !valid(dx) || !valid(dy)) return none();
		const matches: Match[] = [];
		for (const axis of ['x', 'y'] as const) {
			const values = anchors(base, axis),
				xs = anchors(dx, axis),
				ys = anchors(dy, axis);
			const old = this.locks[axis];
			const options: Match[] = [];
			for (const feature of this.features) {
				const a = (xs[feature] - values[feature]) / stepX,
					b = (ys[feature] - values[feature]) / stepY;
				if (Math.abs(a) + Math.abs(b) < 1e-9) continue;
				const held =
					old?.feature === feature &&
					Math.abs(old.value - values[feature]) <= tolerance[axis] * 1.5;
				const value = held ? old.value : nearest(this.coordinates[axis], values[feature]);
				if (value === undefined) continue;
				const delta = value - values[feature],
					distance = Math.abs(delta) / tolerance[axis];
				if (distance <= (held ? 1.5 : 1))
					options.push({ axis, feature, a, b, value, delta, distance });
			}
			const held = options.find((m) => old?.feature === m.feature && old.value === m.value);
			const match =
				held ?? options.sort((a, b) => a.distance - b.distance || a.feature - b.feature)[0];
			if (match) matches.push(match);
		}
		if (!matches.length) return none();
		let x = 0,
			y = 0;
		const [a, b] = matches;
		const det = b ? a.a * b.b - a.b * b.a : 0;
		if (b && Math.abs(det) > 1e-9) {
			x = (a.delta * b.b - a.b * b.delta) / det;
			y = (a.a * b.delta - a.delta * b.a) / det;
		} else {
			const m = matches.toSorted((a, b) => a.distance - b.distance)[0];
			const divisor = (m.a * tolerance.x) ** 2 + (m.b * tolerance.y) ** 2;
			x = (m.delta * m.a * tolerance.x ** 2) / divisor;
			y = (m.delta * m.b * tolerance.y ** 2) / divisor;
		}
		if (!Number.isFinite(x + y) || Math.hypot(x / tolerance.x, y / tolerance.y) > 3) return none();
		const adjusted = { x: point.x + x, y: point.y + y },
			result = this.project(adjusted);
		if (!valid(result)) return none();
		this.clear();
		const guides = matches
			.filter(
				(m) =>
					Math.abs(anchors(result, m.axis)[m.feature] - m.value) <=
					Math.max(1e-7, tolerance[m.axis] * 1e-5)
			)
			.map((m) => {
				this.locks[m.axis] = m;
				return { axis: m.axis, value: m.value };
			});
		return guides.length ? { point: adjusted, guides } : none();
	}
}
