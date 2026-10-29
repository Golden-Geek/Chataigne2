import { describe, expect, it } from 'vitest';
import { RetirementCovers } from '../retirement';

describe('preview retirement coverage', () => {
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
