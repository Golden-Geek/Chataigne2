import {
	LatestEditGesture,
	type GestureHost,
	type GestureEditGroup,
	type LatestEditGestureState
} from 'golden_ui/store/latest-edit-gesture';
import type { InstanceGeometryEdit } from '../media/generated/InstanceGeometryEdit';
import type { InstanceReference } from '../media/generated/InstanceReference';
import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
import type { Point } from '../media/generated/Point';
import type { MediaEdit } from '../media/generated/MediaEdit';
import type { MediaEditResult } from '../media/generated/MediaEditResult';

export type InstanceHandle = Omit<InstanceGeometryEdit, 'point'>;
interface Client<S> {
	group(host: S, handle: InstanceHandle): GestureEditGroup;
	invoke(host: S, edit: MediaEdit): Promise<MediaEditResult>;
	accepted(snapshot: InstanceSnapshot): void;
	active(value: boolean): void;
	finished(): void;
}

/** One typed backend intent per admitted sample; geometry and history semantics stay in Rust. */
export function createInstanceGesture<S extends GestureHost>(
	host: () => S | null,
	state: LatestEditGestureState,
	client: Client<S>
) {
	return new LatestEditGesture<S, InstanceReference, InstanceHandle, Point, InstanceSnapshot>(
		host,
		state,
		{
			...client,
			move: async (host, reference, handle, point) => {
				const result = await client.invoke(host, {
					kind: 'Composition',
					reference: reference.composition,
					edit: {
						kind: 'EditInstanceGeometry',
						instance: reference.instance,
						edit: { ...handle, point }
					}
				});
				if (result.kind !== 'InstanceChanged')
					throw new Error('Unexpected instance geometry result');
				return result.snapshot;
			}
		}
	);
}
