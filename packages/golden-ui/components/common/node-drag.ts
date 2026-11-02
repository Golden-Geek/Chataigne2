import type { NodeId, UiNodeDto } from '../../types';
import type { GraphState } from '../../store/graph.svelte';

export const resolveDragNode = (
	graph: GraphState | null,
	payload: NodeDragPayload | null,
	scope: NodeDragScope | null
): UiNodeDto | null => {
	if (!scope || payload?.scope !== scopeKey(scope)) return null;
	const node = payload ? graph?.nodesById.get(payload.nodeId) : null;
	return node?.uuid === payload?.uuid ? (node ?? null) : null;
};

export const resolveDragParameterNode = (
	graph: GraphState | null,
	payload: NodeDragPayload | null,
	scope: NodeDragScope | null
): UiNodeDto | null => {
	const candidate = resolveDragNode(graph, payload, scope);
	return candidate?.data.kind === 'parameter' ? candidate : null;
};

export const NODE_DRAG_MIME = 'application/x-golden-node';

/** Session identity plus the document replacement generation, independent of graph deltas. */
export interface NodeDragScope {
	readonly snapshotGeneration: number;
}
const owners = new WeakMap<NodeDragScope, string>();
function scopeKey(scope: NodeDragScope): string {
	let owner = owners.get(scope);
	if (!owner) {
		owner = crypto.randomUUID();
		owners.set(scope, owner);
	}
	return `${owner}/${scope.snapshotGeneration}`;
}

export type NodeDragPayload =
	| {
			kind: 'node';
			nodeId: NodeId;
			uuid: string;
			scope: string;
			nodeType: string;
			label: string;
			source: 'outliner';
	  }
	| {
			kind: 'parameter';
			nodeId: NodeId;
			uuid: string;
			scope: string;
			nodeType: string;
			label: string;
			source: 'inspector';
	  };

let activeNodeDragPayload: NodeDragPayload | null = null;

const clearActiveNodeDragPayload = (): void => {
	activeNodeDragPayload = null;
};

const writePayload = (event: DragEvent, payload: NodeDragPayload): void => {
	const transfer = event.dataTransfer;
	if (!transfer) {
		return;
	}

	activeNodeDragPayload = payload;
	if (typeof window !== 'undefined') {
		window.addEventListener('dragend', clearActiveNodeDragPayload, {
			once: true,
			capture: true
		});
	}

	transfer.effectAllowed = 'copyMove';
	transfer.setData(NODE_DRAG_MIME, JSON.stringify(payload));
	transfer.setData('text/plain', payload.label);
	transfer.setData('text/golden-node-id', String(payload.nodeId));
	transfer.setData('text/golden-node-type', payload.nodeType);
	transfer.setData('text/golden-drag-kind', payload.kind);
	transfer.dropEffect = 'copy';
};

export const beginNodeDrag = (
	event: DragEvent,
	node: UiNodeDto,
	scope: NodeDragScope | null
): void => {
	if (!scope) return;
	writePayload(event, {
		kind: 'node',
		nodeId: node.node_id,
		uuid: node.uuid,
		scope: scopeKey(scope),
		nodeType: node.node_type,
		label: node.meta.label,
		source: 'outliner'
	});
};

export const beginParameterDrag = (
	event: DragEvent,
	node: UiNodeDto,
	scope: NodeDragScope | null
): void => {
	if (!scope) return;
	writePayload(event, {
		kind: 'parameter',
		nodeId: node.node_id,
		uuid: node.uuid,
		scope: scopeKey(scope),
		nodeType: node.node_type,
		label: node.meta.label,
		source: 'inspector'
	});
};

export const readNodeDragPayload = (event: DragEvent): NodeDragPayload | null => {
	const raw = event.dataTransfer?.getData(NODE_DRAG_MIME) ?? '';
	if (raw.length === 0) {
		// Native dragover protects payload data; only same-window typed drags may use this cache.
		return event.dataTransfer?.types.includes(NODE_DRAG_MIME) ? activeNodeDragPayload : null;
	}

	try {
		const parsed = JSON.parse(raw) as Partial<NodeDragPayload>;
		if (
			(parsed.kind !== 'node' && parsed.kind !== 'parameter') ||
			typeof parsed.nodeId !== 'number' ||
			!Number.isSafeInteger(parsed.nodeId) ||
			typeof parsed.uuid !== 'string' ||
			parsed.uuid.length === 0 ||
			typeof parsed.scope !== 'string' ||
			typeof parsed.nodeType !== 'string' ||
			typeof parsed.label !== 'string'
		) {
			return null;
		}

		if (parsed.kind === 'node') {
			return {
				kind: 'node',
				nodeId: parsed.nodeId,
				uuid: parsed.uuid,
				scope: parsed.scope,
				nodeType: parsed.nodeType,
				label: parsed.label,
				source: 'outliner'
			};
		}

		return {
			kind: 'parameter',
			nodeId: parsed.nodeId,
			uuid: parsed.uuid,
			scope: parsed.scope,
			nodeType: parsed.nodeType,
			label: parsed.label,
			source: 'inspector'
		};
	} catch {
		return null;
	}
};
