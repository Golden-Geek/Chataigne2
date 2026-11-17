import type { OverlayPrimitive } from 'golden_video_ui';
import type { Point } from './generated/Point';
import { handleOverlays, imageFrameOverlay, quadOverlay } from './overlays.ts';

/** Native paint and hit admission follow the same crop selection as the DOM controls. */
export function cropOverlays(
	corners: readonly Point[],
	selected: boolean,
	ghost: Point | null,
	rem: number,
	hovered = false,
	focusedCorner: number | null = null
): OverlayPrimitive[] {
	return [
		imageFrameOverlay(rem),
		quadOverlay(corners, rem, { selected, hovered, hit: 'crop' }),
		...(selected
			? corners.flatMap((point, index) =>
					handleOverlays(point, rem, focusedCorner === index, false, `crop:${index}`)
				)
			: []),
		...(selected && ghost ? handleOverlays(ghost, rem, false, true) : [])
	];
}
