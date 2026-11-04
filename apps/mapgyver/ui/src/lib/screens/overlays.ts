import type { OverlayPrimitive } from 'golden_video_ui';
import type { Point } from './generated/Point';

const color = (hex: string): [number, number, number, number] => [
	Number.parseInt(hex.slice(0, 2), 16) / 255,
	Number.parseInt(hex.slice(2, 4), 16) / 255,
	Number.parseInt(hex.slice(4, 6), 16) / 255,
	hex.length === 8 ? Number.parseInt(hex.slice(6, 8), 16) / 255 : 1
];
export const overlayRem = () =>
	Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;

export function imageFrameOverlay(rem: number): OverlayPrimitive {
	return {
		hit: null,
		shape: {
			kind: 'Quad',
			corners: [
				[0, 0],
				[1, 0],
				[1, 1],
				[0, 1]
			]
		},
		fill: [0, 0, 0, 0],
		stroke: color('666666'),
		width: 0.0625 * rem,
		dash: null
	};
}

export function quadOverlay(
	corners: readonly Point[],
	rem: number,
	{ selected = false, enabled = true, valid = true, hit = null as string | null } = {}
): OverlayPrimitive {
	return {
		hit,
		shape: {
			kind: 'Quad',
			corners: corners.map((p) => [p.x, p.y]) as [
				[number, number],
				[number, number],
				[number, number],
				[number, number]
			]
		},
		fill: color(selected ? (!valid ? 'a51d2c18' : '78aafa18') : '00000000'),
		stroke: color(!valid ? 'ff5364' : selected ? '70aaff' : 'a2b0c5'),
		width: (selected ? 0.125 : 0.0625) * rem,
		dash: enabled ? null : [0.3 * rem, 0.2 * rem]
	};
}

export function handleOverlays(
	point: Point,
	rem: number,
	selected = false,
	ghost = false,
	hit: string | null = null
): OverlayPrimitive[] {
	const shape = {
		kind: 'Circle' as const,
		center: [point.x, point.y] as [number, number],
		radius: 0.375 * rem
	};
	const handle: OverlayPrimitive = {
		hit,
		shape,
		fill: color(ghost ? '00000000' : selected ? 'a3bfff' : 'dceaff'),
		stroke: color(ghost ? 'ffe19b' : selected ? 'dddddd' : '467bcc'),
		width: 0.1 * rem,
		dash: null
	};
	return selected
		? [
				{
					...handle,
					hit: null,
					shape: { ...shape, radius: 0.475 * rem },
					fill: color('a3bfff'),
					stroke: color('a3bfff')
				},
				handle
			]
		: [handle];
}
