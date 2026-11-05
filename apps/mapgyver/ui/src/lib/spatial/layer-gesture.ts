import {
	LatestEditGesture,
	type GestureHost,
	type GestureEditGroup,
	type LatestEditGestureState
} from 'golden_ui/store/latest-edit-gesture';
import type { RectGeometryEdit } from '../media/generated/RectGeometryEdit';
import type { Point } from '../media/generated/Point';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';
export type LayerHandle = Omit<RectGeometryEdit, 'point'>;
export type LayerGeometryResult = Extract<MediaEditResult, { kind: 'MatrixLayerGeometryChanged' }>;
export function createLayerGesture<S extends GestureHost>(
	host: () => S | null,
	state: LatestEditGestureState,
	client: {
		group(host: S, handle: LayerHandle): GestureEditGroup;
		invoke(host: S, edit: MediaEdit): Promise<MediaEditResult>;
		accepted(result: LayerGeometryResult): void;
		active(active: boolean): void;
		finished(): void;
	}
) {
	return new LatestEditGesture<
		S,
		{ media: string; layer: string },
		LayerHandle,
		Point,
		LayerGeometryResult
	>(host, state, {
		...client,
		move: async (host, reference, handle, point) => {
			const result = await client.invoke(host, {
				kind: 'Matrix',
				media: reference.media,
				edit: { kind: 'EditLayerGeometry', layer: reference.layer, edit: { ...handle, point } }
			});
			if (
				result.kind !== 'MatrixLayerGeometryChanged' ||
				result.media !== reference.media ||
				result.layer !== reference.layer
			)
				throw new Error('Unexpected Layer geometry result');
			return result;
		}
	});
}
