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
	readonly editOperations: Pick<ParameterPreviews, 'get' | 'enabled'>;
}

/** Read-only presentation of backend-bound Parameters. Drafts precede transport delivery;
 * committed values must reach the read snapshot's event boundary before replacing it.
 * This never changes the graph, chooses domain defaults or searches a document subtree. */
export function editorParameterProjection(
	host: EditorParameterProjectionHost | null,
	generation: number | null,
	at: EventTime
) {
	const lookup = (uuid: string) => {
		if (
			!host ||
			host.status !== 'connected' ||
			host.loading.visible ||
			host.snapshotGeneration !== generation
		)
			return;
		const graph = host.graph.state;
		if (graph.requiresResync) return;
		const node = graph.nodesById.getByUuid(uuid);
		if (!node || node.data.kind !== 'parameter') return;
		return {
			host,
			graph,
			node,
			committed: graph.lastEventTime !== undefined && compareEventTime(graph.lastEventTime, at) >= 0
		};
	};
	const read = <K extends keyof ValuesByKind>(
		uuid: string,
		kind: K
	): ValuesByKind[K] | undefined => {
		const current = lookup(uuid);
		if (!current) return;
		const { host, graph, node, committed } = current;
		const parameter =
			host.editOperations.get(node.node_id, uuid) ??
			(committed ? graph.paramsById.get(node.node_id)?.value : undefined);
		return parameter?.kind === kind && 'value' in parameter
			? (parameter.value as ValuesByKind[K])
			: undefined;
	};
	return {
		read,
		enabled(uuid: string, fallback: boolean): boolean {
			const current = lookup(uuid);
			if (!current) return fallback;
			return (
				current.host.editOperations.enabled(current.node.node_id, uuid) ??
				(current.committed ? current.node.meta.enabled : fallback)
			);
		},
		value<K extends keyof ValuesByKind>(
			uuid: string,
			kind: K,
			fallback: ValuesByKind[K]
		): ValuesByKind[K] {
			return read(uuid, kind) ?? fallback;
		}
	};
}
