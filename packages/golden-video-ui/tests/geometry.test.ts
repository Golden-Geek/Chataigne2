import { describe, expect, it } from 'vitest';
import { clipPreviewRect, intersectRect } from '../geometry';

describe('CSS preview geometry', () => {
	it('clips both axes through nested scroll containers without changing the source rectangle', () => {
		const source = { x: -20, y: 10, width: 100, height: 80 };
		const clip = clipPreviewRect(source, { x: 0, y: 0, width: 120, height: 90 }, [
			{ rect: { x: 5, y: 20, width: 60, height: 50 }, clipX: true, clipY: false },
			{ rect: { x: 0, y: 30, width: 100, height: 20 }, clipX: false, clipY: true }
		]);
		expect(clip).toEqual({ x: 5, y: 30, width: 60, height: 20 });
		expect(source).toEqual({ x: -20, y: 10, width: 100, height: 80 });
	});

	it('keeps fractional CSS coordinates and visual viewport origin without applying DPR', () => {
		expect(
			clipPreviewRect(
				{ x: 1.5, y: 4.25, width: 20.5, height: 10.5 },
				{ x: 2.75, y: 3, width: 10.5, height: 12 },
				[]
			)
		).toEqual({ x: 2.75, y: 4.25, width: 10.5, height: 10.5 });
	});

	it('returns empty visible geometry when the entire source is outside the clipping viewport', () => {
		expect(
			intersectRect(
				{ x: 100, y: 100, width: 20, height: 20 },
				{ x: 0, y: 0, width: 10, height: 10 }
			)
		).toEqual({ x: 100, y: 100, width: 0, height: 0 });
	});
});
