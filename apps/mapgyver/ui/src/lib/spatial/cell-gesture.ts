import {
	LatestEditGesture,
	type GestureHost,
	type GestureEditGroup,
	type LatestEditGestureState
} from 'golden_ui/store/latest-edit-gesture';
import type { CellGeometryEdit } from '../media/generated/CellGeometryEdit';
import type { Point } from '../media/generated/Point';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
type WithoutPoint<T> = T extends unknown ? Omit<T, 'point'> : never;
export type CellHandle = WithoutPoint<CellGeometryEdit>;
export type CellGeometryResult = Extract<MediaEditResult, { kind: 'MatrixCellGeometryChanged' }>;
export function createCellGesture<S extends GestureHost>(
	host: () => S | null,
	state: LatestEditGestureState,
	client: {
		group(host: S, handle: CellHandle): GestureEditGroup;
		invoke(host: S, edit: MediaEdit): Promise<MediaEditResult>;
		accepted(result: CellGeometryResult): void;
		active(active: boolean): void;
		finished(): void;
	}
) {
	return new LatestEditGesture<
		S,
		{ media: string; cell: string },
		CellHandle,
		Point,
		CellGeometryResult
	>(host, state, {
		...client,
		move: async (host, reference, handle, point) => {
			const result = await client.invoke(host, {
				kind: 'Matrix',
				media: reference.media,
				edit: { kind: 'EditCellGeometry', cell: reference.cell, edit: { ...handle, point } }
			});
			if (
				result.kind !== 'MatrixCellGeometryChanged' ||
				result.media !== reference.media ||
				result.cell !== reference.cell
			)
				throw new Error('Unexpected Cell geometry result');
			return result;
		}
	});
}
