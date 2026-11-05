import { KeyboardGesture } from 'golden_ui/store/keyboard-gesture';
import type { Point } from './generated/Point';
import type { PixelOffset } from './generated/PixelOffset';
import type { SurfaceReference } from './generated/SurfaceReference';

export interface KeyboardTarget {
	document: string;
	reference: SurfaceReference;
	corner: number;
	origin: Point;
}

/** Screen-specific identity and immutable corner basis over Golden's held-key lifecycle. */
export const createSurfaceKeyboard = (client: {
	start(target: KeyboardTarget): boolean;
	move(pixels: PixelOffset): void;
	finish(): void;
	stop(): void;
}) =>
	new KeyboardGesture<KeyboardTarget>({
		...client,
		identity: (target) =>
			JSON.stringify([
				target.document,
				target.reference.screen,
				target.reference.surface,
				target.corner
			]),
		start: (target) =>
			client.start({ ...target, reference: { ...target.reference }, origin: { ...target.origin } })
	});
