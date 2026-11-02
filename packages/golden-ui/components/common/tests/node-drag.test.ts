import { describe, expect, test } from 'vitest';
import { beginNodeDrag, NODE_DRAG_MIME, readNodeDragPayload, resolveDragNode } from '../node-drag';
import type { UiNodeDto } from '../../../types';
import type { GraphState } from '../../../store/graph.svelte';

function transfer() {
	const data = new Map<string, string>();
	const value = {
		get types() {
			return [...data.keys()];
		},
		getData: (type: string) => data.get(type) ?? '',
		setData: (type: string, text: string) => {
			data.set(type, text);
		}
	} as DataTransfer;
	return { data, value, event: { dataTransfer: value } as DragEvent };
}

const node = {
	node_id: 42,
	uuid: 'original-project-object',
	node_type: 'source',
	meta: { label: 'Source' }
} as UiNodeDto;
const scope = { snapshotGeneration: 1 };

describe('document node drag identity', () => {
	test('protected dragover data uses the current typed drag but unrelated external data cannot', () => {
		const drag = transfer();
		beginNodeDrag(drag.event, node, scope);
		const protectedEvent = {
			dataTransfer: { types: [NODE_DRAG_MIME], getData: () => '' }
		} as unknown as DragEvent;
		expect(readNodeDragPayload(protectedEvent)?.uuid).toBe(node.uuid);
		expect(readNodeDragPayload(transfer().event)).toBeNull();
	});

	test('a drag cannot bind a reused node ID from a replacement project', () => {
		const drag = transfer();
		beginNodeDrag(drag.event, node, scope);
		const payload = readNodeDragPayload(drag.event);
		const graph = { nodesById: new Map([[node.node_id, node]]) } as GraphState;
		expect(resolveDragNode(graph, payload, scope)).toBe(node);
		graph.nodesById.set(node.node_id, { ...node, uuid: 'replacement-project-object' });
		expect(resolveDragNode(graph, payload, scope)).toBeNull();
		graph.nodesById.clear();
		expect(resolveDragNode(graph, payload, scope)).toBeNull();
	});

	test('malformed payloads cannot fall back to a previously active drag', () => {
		const drag = transfer();
		beginNodeDrag(drag.event, node, scope);
		drag.data.set(NODE_DRAG_MIME, '{');
		expect(readNodeDragPayload(drag.event)).toBeNull();
		drag.data.set(
			NODE_DRAG_MIME,
			JSON.stringify({ kind: 'node', nodeId: 42, nodeType: 'source', label: 'Old payload' })
		);
		expect(readNodeDragPayload(drag.event)).toBeNull();
	});

	test('reopening identical saved UUIDs and changing sessions invalidate raw and protected drags', () => {
		const drag = transfer();
		const session = { snapshotGeneration: 7 };
		beginNodeDrag(drag.event, node, session);
		const graph = { nodesById: new Map([[node.node_id, node]]) } as GraphState;
		const payload = readNodeDragPayload(drag.event);
		expect(resolveDragNode(graph, payload, session)).toBe(node);
		expect(resolveDragNode(graph, payload, { snapshotGeneration: 7 })).toBeNull();
		session.snapshotGeneration++;
		expect(resolveDragNode(graph, readNodeDragPayload(drag.event), session)).toBeNull();
		const protectedEvent = {
			dataTransfer: { types: [NODE_DRAG_MIME], getData: () => '' }
		} as unknown as DragEvent;
		expect(resolveDragNode(graph, readNodeDragPayload(protectedEvent), session)).toBeNull();
	});
});
