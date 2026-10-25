import { describe, expect, it } from 'vitest';
import { clipFrameBounds } from '../frame-bounds';

const viewport = { left: 100, top: 200, width: 300, height: 400 };
describe('dashboard framing inside clipped surfaces', () => {
	it('reads inherited coordinates like native DOMRect instead of spreading enumerable fields', () => {
		class Rectangle {
			get left() {
				return 100;
			}
			get top() {
				return 220;
			}
			get width() {
				return 60;
			}
			get height() {
				return 40;
			}
		}
		expect(clipFrameBounds(new Rectangle(), viewport, false, false)).toEqual({
			left: 100,
			top: 220,
			width: 60,
			height: 40
		});
	});
	it('frames a visible partial control rather than the offscreen portion', () => {
		expect(
			clipFrameBounds({ left: 50, top: 250, width: 100, height: 40 }, viewport, true, false)
		).toEqual({ left: 100, top: 250, width: 50, height: 40 });
	});
	it('represents offscreen scroll contents by the containing viewport', () => {
		expect(
			clipFrameBounds({ left: 1000, top: -1000, width: 40, height: 40 }, viewport, true, true)
		).toEqual(viewport);
	});
	it('preserves unconstrained axes and un-clipped free geometry', () => {
		const rect = { left: -2000, top: 0, width: 10, height: 20 };
		expect(clipFrameBounds(rect, viewport, false, false)).toEqual(rect);
		expect(clipFrameBounds(rect, viewport, false, true)).toEqual({
			...rect,
			top: 200,
			height: 400
		});
	});
});
