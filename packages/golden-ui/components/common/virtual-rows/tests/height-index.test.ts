import { describe, expect, it } from 'vitest';
import { RowHeightIndex } from '../height-index';

describe('keyed virtual row geometry', () => {
	it('keeps bounded windows at the beginning, middle and clamped end of 100k rows', () => {
		const index = new RowHeightIndex();
		index.synchronize(
			Array.from({ length: 100000 }, (_, key) => key),
			24
		);
		expect(index.total).toBe(2400000);
		expect(index.window(0, 240)).toEqual({ start: 0, end: 19 });
		expect(index.window(1200000, 240)).toEqual({ start: 49992, end: 50019 });
		expect(index.window(1e10, 240)).toEqual({ start: 99982, end: 100000 });
		expect(index.rowAt(index.total)).toBe(99999);
	});
	it('retains measured identities through reorder/filter while updating exact offsets', () => {
		const index = new RowHeightIndex();
		index.synchronize(['a', 'b', 'c'], 20);
		index.measure('b', 75);
		expect(index.offset(2)).toBe(95);
		expect(index.rowAt(94.9)).toBe(1);
		expect(index.rowAt(95)).toBe(2);
		index.synchronize(['c', 'b'], 30);
		expect(index.offset(1)).toBe(30);
		expect(index.total).toBe(105);
		expect(index.indexOf('b')).toBe(1);
		expect(index.synchronize(['c', 'b'], 30)).toBe(false);
		expect(index.measure('missing', 20)).toBe(false);
		index.synchronize([], 30);
		expect(index.total).toBe(0);
		expect(index.window(200, 100)).toEqual({ start: 0, end: 0 });
	});
	it('matches the linear oracle after sparse height changes', () => {
		const index = new RowHeightIndex(),
			heights = Array<number>(10000).fill(20);
		index.synchronize(
			heights.map((_, key) => key),
			20
		);
		for (let change = 0; change < 64; change += 1) {
			const key = (change * 157) % heights.length;
			heights[key] = 30 + change;
			index.measure(key, heights[key]);
		}
		let prefix = 0;
		for (let row = 0; row < heights.length; row += 1) {
			if (row % 127 === 0) {
				expect(index.offset(row)).toBe(prefix);
				expect(index.rowAt(prefix + heights[row] / 2)).toBe(row);
			}
			prefix += heights[row];
		}
		expect(index.total).toBe(prefix);
	});
	it('rejects invalid geometry and duplicate identities before replacing the index', () => {
		const index = new RowHeightIndex();
		index.synchronize(['a'], 20);
		expect(() => index.synchronize(['b', 'b'], 20)).toThrow(/Duplicate/);
		expect(index.indexOf('a')).toBe(0);
		expect(() => index.synchronize(['a'], NaN)).toThrow(RangeError);
		expect(() => index.measure('a', 0)).toThrow(RangeError);
		expect(index.total).toBe(20);
	});
});
