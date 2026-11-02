import { describe, expect, it } from 'vitest';
import { RetirementCovers, subtractRect } from '../retirement';

describe('preview retirement coverage', () => {
	it('covers only vacated and newly exposed areas around a retained image', () => {
		const covered = { x: 0, y: 0, width: 100, height: 80 };
		const retained = { x: 20, y: 10, width: 60, height: 50 };
		const pieces = subtractRect(covered, retained);
		expect(pieces).toHaveLength(4);
		expect(pieces.reduce((sum, rect) => sum + rect.width * rect.height, 0)).toBe(5000);
		for (const rect of pieces)
			expect(
				rect.x + rect.width <= retained.x ||
					rect.x >= retained.x + retained.width ||
					rect.y + rect.height <= retained.y ||
					rect.y >= retained.y + retained.height
			).toBe(true);
		expect(subtractRect(covered, covered)).toEqual([]);
		expect(subtractRect(covered, { ...retained, x: 200 })).toEqual([covered]);
		expect(subtractRect(covered)).toEqual([covered]);
	});
	it('unions old clipped areas for one view until confirmed replacement', () => {
		const covers = new RetirementCovers();
		covers.protect('view', { x: 10, y: 20, width: 30, height: 40 });
		covers.protect('view', { x: 0, y: 30, width: 20, height: 50 });
		expect(covers.entries()).toEqual([['view', { x: 0, y: 20, width: 40, height: 60 }]]);
		covers.clear();
		expect(covers.size).toBe(0);
	});
	it('retains conservative coverage without unbounded masks during rapid unmount/remount churn', () => {
		const covers = new RetirementCovers(2);
		for (let index = 0; index < 1000; index++)
			covers.protect(`view-${index}`, { x: index, y: 0, width: 2, height: 2 });
		expect(covers.size).toBe(2);
		const rects = covers.entries().map(([, rect]) => rect);
		for (let index = 0; index < 1000; index++)
			expect(rects.some((rect) => rect.x <= index && rect.x + rect.width >= index + 2)).toBe(true);
	});
	it('ignores empty and nonfinite measurement without allocating a historical mask', () => {
		const covers = new RetirementCovers(1);
		covers.protect('empty', { x: 0, y: 0, width: 0, height: 5 });
		covers.protect('invalid', { x: NaN, y: 0, width: 5, height: 5 });
		expect(covers.size).toBe(0);
	});
});
