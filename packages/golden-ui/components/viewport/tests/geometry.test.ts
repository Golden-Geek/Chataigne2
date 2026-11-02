import { describe, expect, it } from 'vitest';
import { fitSize, frameBounds, zoomAt } from '../geometry';

describe('bounded editor camera adapter', () => {
	it('fits the active rectangle without distorting non-square documents', () => {
		expect(fitSize(768, 480, 2)).toEqual({ width: 768, height: 384 });
		expect(fitSize(0, 480, 2)).toEqual({ width: 0, height: 0 });
	});
	it('frames normalized selection with the shared camera margins and viewport dimensions', () => {
		const camera = frameBounds(
			{ x: 0.2, y: 0.2, width: 0.2, height: 0.25 },
			{ width: 768, height: 384 },
			{ width: 768, height: 480 },
			48
		)!;
		expect(camera.zoom).toBe(4);
		expect(camera.pan.x + (0.3 - 0.5) * 768 * camera.zoom).toBeCloseTo(0);
		expect(camera.pan.y + (0.325 - 0.5) * 384 * camera.zoom).toBeCloseTo(0);
		expect(
			frameBounds(
				{ x: 0, y: 0, width: 0, height: 1 },
				{ width: 768, height: 384 },
				{ width: 768, height: 480 },
				48
			)
		).toBeNull();
	});
	it('keeps the cursor anchor fixed when adapting the public camera shape', () => {
		expect(zoomAt({ zoom: 1, pan: { x: 40, y: -20 } }, 2, { x: 140, y: 80 })).toEqual({
			zoom: 2,
			pan: { x: -60, y: -120 }
		});
	});
});
