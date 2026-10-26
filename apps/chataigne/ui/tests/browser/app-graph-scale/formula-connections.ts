import type { GraphState } from 'golden_ui/store/graph.svelte';
import type { UiNodeDto } from 'golden_ui';

interface Connection {
	id: number;
	uuid: string;
	source: string;
	target: string | null;
	sourceSocket: string;
	targetSocket: string;
}

/** Read-only measurement index; reconcile actual backend records without scanning every frame. */
export function createNativeFormulaConnections() {
	let previous: GraphState | undefined;
	const connections = new Map<number, Connection>();
	const outgoing = new Map<string, Map<number, Connection>>();
	const remove = (id: number) => {
		const old = connections.get(id);
		if (!old) return;
		const group = outgoing.get(old.source)!;
		group.delete(id);
		if (!group.size) outgoing.delete(old.source);
		connections.delete(id);
	};
	return (graph: GraphState, source: string): Connection[] => {
		if (previous !== graph) {
			const dirty = new Set<number>();
			const visit = (id: number, node?: UiNodeDto, old?: UiNodeDto) => {
				if ([node, old].some((node) => node?.node_type === 'alchemist_connection')) dirty.add(id);
				for (const parent of [graph.parentById.get(id), previous?.parentById.get(id)]) {
					if (
						parent !== undefined &&
						[graph.nodesById.get(parent), previous?.nodesById.get(parent)].some(
							(node) => node?.node_type === 'alchemist_connection'
						)
					)
						dirty.add(parent);
				}
			};
			if (previous) graph.nodesById.forEachChange(previous.nodesById, visit);
			else graph.nodesById.forEach((node, id) => visit(id, node));
			for (const id of dirty) {
				remove(id);
				const node = graph.nodesById.get(id);
				if (node?.node_type !== 'alchemist_connection') continue;
				const fields = new Map(
					node.children.map((id) => {
						const child = graph.nodesById.get(id);
						return [
							child?.decl_id,
							child?.data.kind === 'parameter' ? child.data.param.value : null
						];
					})
				);
				const from = fields.get('source_node'),
					to = fields.get('target_node');
				const output = fields.get('source_socket'),
					input = fields.get('target_socket');
				if (
					from?.kind !== 'reference' ||
					to?.kind !== 'reference' ||
					output?.kind !== 'str' ||
					input?.kind !== 'str'
				)
					continue;
				const edge = {
					id,
					uuid: node.uuid,
					source: from.uuid,
					target: to.uuid,
					sourceSocket: output.value,
					targetSocket: input.value
				};
				connections.set(id, edge);
				const group = outgoing.get(edge.source) ?? new Map<number, Connection>();
				group.set(id, edge);
				outgoing.set(edge.source, group);
			}
			previous = graph;
		}
		return [...(outgoing.get(source)?.values() ?? [])];
	};
}
