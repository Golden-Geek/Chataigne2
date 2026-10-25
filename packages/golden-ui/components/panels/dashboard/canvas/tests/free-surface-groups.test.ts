import { describe, expect, it } from 'vitest';
import { FreeSurfaceGroups } from '../free-surface-groups';
import { rectsIntersect } from '../geometry';
import type { FreeLayoutRect } from '../contracts';

const rect = (left: number, top = 0): FreeLayoutRect => ({ left, top, width: 10, height: 10 });
const contains = (outer: FreeLayoutRect, inner: FreeLayoutRect) =>
	outer.left <= inner.left &&
	outer.top <= inner.top &&
	outer.left + outer.width >= inner.left + inner.width &&
	outer.top + outer.height >= inner.top + inner.height;

describe('retained dashboard overview groups', () => {
	it('covers all 100k overlapping members with a fixed number of authored bounds', () => {
		const groups = new FreeSurfaceGroups();
		groups.rebuild(Array.from({ length: 100_000 }, (_, id) => [id, rect(0)] as const));
		const visible = groups.query(rect(0), 256);
		expect(visible.length).toBeLessThanOrEqual(256);
		expect(visible.reduce((total, group) => total + group.count, 0)).toBe(100_000);
		expect(visible.every((group) => contains(group.rect, rect(0)))).toBe(true);
	});
	it('covers every visible member of a grid, including boundaries and negative coordinates', () => {
		const groups = new FreeSurfaceGroups();
		const entries = Array.from(
			{ length: 4096 },
			(_, id) => [id, rect((id % 64) * 20 - 640, Math.floor(id / 64) * 20 - 640)] as const
		);
		groups.rebuild(entries);
		const viewport = { left: -99, top: -61, width: 401, height: 281 };
		const visible = groups.query(viewport, 32);
		expect(visible.length).toBeLessThanOrEqual(32);
		for (const [, member] of entries.filter(([, member]) => rectsIntersect(viewport, member)))
			expect(visible.some((group) => contains(group.rect, member))).toBe(true);
	});
	it('repairs moved bounds and retires all state on rebuild and clear', () => {
		const groups = new FreeSurfaceGroups();
		groups.rebuild([
			[1, rect(0)],
			[2, rect(100)],
			[3, rect(200)]
		]);
		groups.update(2, rect(-1000));
		expect(groups.query(rect(-1000), 16)).toEqual([{ rect: rect(-1000), count: 1 }]);
		expect(groups.query(rect(100), 16)).toEqual([]);
		groups.rebuild([[1, rect(0)]]);
		expect(groups.query(rect(-1000), 16)).toEqual([]);
		groups.clear();
		expect(groups.query(rect(0), 16)).toEqual([]);
		groups.rebuild([]);
		expect(groups.query(rect(0), 1)).toEqual([]);
	});
	it('rejects invalid budgets and conservatively supports a single bound', () => {
		const groups = new FreeSurfaceGroups();
		groups.rebuild([
			[1, rect(0)],
			[2, rect(20)]
		]);
		expect(groups.query({ left: 0, top: 0, width: 30, height: 10 }, 1)).toEqual([
			{ rect: { left: 0, top: 0, width: 30, height: 10 }, count: 2 }
		]);
		for (const budget of [0, -1, 1.5, Infinity])
			expect(() => groups.query(rect(0), budget)).toThrow(RangeError);
	});
});
