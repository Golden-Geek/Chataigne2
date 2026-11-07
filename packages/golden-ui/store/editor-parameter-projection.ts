import type { EventTime, ParamValue } from '../types';
import type { GraphState } from './graph.svelte';
import type { ParameterPreviews } from './session/edit-operations';
import { compareEventTime } from './session/event-time.ts';

type ValueParameters = Extract<ParamValue, { value: unknown }>;
type ValuesByKind = { [P in ValueParameters as P['kind']]: P['value'] };

export interface EditorParameterProjectionHost {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
	readonly graph: { readonly state: GraphState };
	readonly editOperations: Pick<ParameterPreviews, 'get'>;
}

/** Read-only presentation of backend-bound Parameters. Drafts precede transport delivery;
 * committed values must reach the read snapshot's event boundary before replacing it.
 * This never changes the graph, chooses domain defaults or searches a document subtree. */
export function editorParameterProjection(
	host: EditorParameterProjectionHost | null,
	generation: number | null,
	at: EventTime
) {
	return {
		value<K extends keyof ValuesByKind>(
			uuid: string,
			kind: K,
			fallback: ValuesByKind[K]
		): ValuesByKind[K] {
			if (
				!host ||
				host.status !== 'connected' ||
				host.loading.visible ||
				host.snapshotGeneration !== generation
			)
				return fallback;
			const graph = host.graph.state;
			if (graph.requiresResync) return fallback;
			const node = graph.nodesById.getByUuid(uuid);
			if (!node || node.data.kind !== 'parameter') return fallback;
			const draft = host.editOperations.get(node.node_id, uuid);
			const parameter =
				draft ??
				(graph.lastEventTime && compareEventTime(graph.lastEventTime, at) >= 0
					? graph.paramsById.get(node.node_id)?.value
					: undefined);
			return parameter?.kind === kind && 'value' in parameter
				? (parameter.value as ValuesByKind[K])
				: fallback;
		}
	};
}
