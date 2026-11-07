import {
	LatestEditGesture,
	createLatestEditGestureState,
	type GestureHost,
	type GestureEditGroup,
	type LatestEditGestureState
} from 'golden_ui/store/latest-edit-gesture';
import type { Point } from './generated/Point';
import type { PixelOffset } from './generated/PixelOffset';
import type { SurfaceReference } from './generated/SurfaceReference';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { SurfaceGeometryEdit } from './generated/SurfaceGeometryEdit';

type Handle<T> = T extends { point: Point }
	? Omit<T, 'point'>
	: T extends { pixels: PixelOffset }
		? Omit<T, 'pixels'>
		: never;
export type SurfaceHandle = Handle<SurfaceGeometryEdit>;
export type SurfacePresentation = import('golden_ui/store/latest-edit-gesture').GesturePresentation<
	SurfaceReference,
	SurfaceHandle,
	Point
>;
export type EditGroup = GestureEditGroup;
export type SurfaceGestureState = LatestEditGestureState;
export const createSurfaceGestureState = createLatestEditGestureState;
interface Client<S> {
	group(host: S, handle: SurfaceHandle): EditGroup;
	move(host: S, reference: SurfaceReference, edit: SurfaceGeometryEdit): Promise<SurfaceSnapshot>;
	accepted(snapshot: SurfaceSnapshot): void;
	active(value: boolean): void;
	finished(): void;
	present?(value: SurfacePresentation | null): void;
}

/** App adapter; generic bounded ownership/history lifecycle belongs to Golden UI. */
export class SurfaceGesture<S extends GestureHost> extends LatestEditGesture<
	S,
	SurfaceReference,
	SurfaceHandle,
	Point,
	SurfaceSnapshot
> {
	private kind: SurfaceHandle['kind'] | null = null;
	private gestureState: SurfaceGestureState;
	constructor(host: () => S | null, state: SurfaceGestureState, client: Client<S>) {
		super(host, state, {
			...client,
			move: (host, reference, handle, point) =>
				client.move(
					host,
					reference,
					handle.kind === 'QuadCornerPixels' ? { ...handle, pixels: point } : { ...handle, point }
				)
		});
		this.gestureState = state;
	}
	override start(reference: SurfaceReference, handle: SurfaceHandle): boolean {
		const started = super.start(reference, handle);
		if (started) this.kind = handle.kind;
		return started;
	}
	get pixelsActive() {
		return this.gestureState.active && this.kind === 'QuadCornerPixels';
	}
}
