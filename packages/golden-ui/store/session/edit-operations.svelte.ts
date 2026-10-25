import type { GraphStore } from '../graph.svelte';
import { createEditOperations } from './edit-operations';

export function createSessionEditOperations(graph: GraphStore) {
	let revision = $state(0),
		queued = false;
	return createEditOperations({
		uuid: (id) => graph.state.nodesById.get(id)?.uuid,
		eventTime: () => graph.state.lastEventTime,
		track: () => {
			void revision;
		},
		changed: () => {
			// View disposal can happen inside keyed rendering; publish after that DOM update.
			if (queued) return;
			queued = true;
			queueMicrotask(() => {
				queued = false;
				revision++;
			});
		}
	});
}
