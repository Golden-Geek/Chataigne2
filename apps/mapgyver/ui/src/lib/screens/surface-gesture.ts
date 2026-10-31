import {
	LatestEditGesture,
	createLatestEditGestureState,
	type GestureHost,
	type GestureEditGroup,
	type LatestEditGestureState
} from 'golden_ui/store/latest-edit-gesture';
import type { Point } from './generated/Point';
import type { SurfaceReference } from './generated/SurfaceReference';
import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
import type { SurfaceGeometryEdit } from './generated/SurfaceGeometryEdit';

type Handle<T> = T extends { point: Point } ? Omit<T, 'point'> : never;
export type SurfaceHandle = Handle<SurfaceGeometryEdit>;
export type EditGroup = GestureEditGroup;
export type SurfaceGestureState = LatestEditGestureState;
export const createSurfaceGestureState = createLatestEditGestureState;
interface Client<S> {
	group(host: S, handle: SurfaceHandle): EditGroup;
	move(host: S, reference: SurfaceReference, edit: SurfaceGeometryEdit): Promise<SurfaceSnapshot>;
	accepted(snapshot: SurfaceSnapshot): void;
	active(value: boolean): void;
	finished(): void;
}

/** App adapter; generic bounded ownership/history lifecycle belongs to Golden UI. */
export class SurfaceGesture<S extends GestureHost> extends LatestEditGesture<
	S,
	SurfaceReference,
	SurfaceHandle,
	Point,
	SurfaceSnapshot
> {
	constructor(host: () => S | null, state: SurfaceGestureState, client: Client<S>) {
		super(host, state, {
			...client,
			move: (host, reference, handle, point) => client.move(host, reference, { ...handle, point })
		});
	}
}
