import type { OverlayPrimitive } from 'golden_video_ui';
import type { Point } from './generated/Point';

/** Same screen-sized chain glyph and hit region as the DOM button, at any viewport zoom. */
export function linkOverlays(
	point: Point,
	frame: { width: number; height: number },
	rem: number,
	hit: string,
	missing: boolean
): OverlayPrimitive[] {
	if (!(frame.width > 0 && frame.height > 0)) return [];
	const x = point.x + (1.2 * rem) / frame.width,
		y = point.y;
	const stroke: [number, number, number, number] = missing ? [1, 0.8, 0.45, 1] : [0.64, 0.75, 1, 1];
	return [
		{
			hit,
			shape: { kind: 'Circle', center: [x, y], radius: 0.6 * rem },
			fill: [0.125, 0.15, 0.2, 0.95],
			stroke: [0, 0, 0, 0],
			width: 0,
			dash: null
		},
		...[-1, 1].map((direction): OverlayPrimitive => ({
			hit,
			shape: {
				kind: 'Circle',
				center: [
					x + (direction * 0.125 * rem) / frame.width,
					y + (direction * 0.125 * rem) / frame.height
				],
				radius: (5 / 24) * rem
			},
			fill: [0, 0, 0, 0],
			stroke,
			width: (1.5 / 24) * rem,
			dash: null
		}))
	];
}
