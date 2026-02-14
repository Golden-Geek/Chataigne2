import { writable, type Readable } from 'svelte/store';
import type {
	EventTime,
	NodeId,
	UiEventBatch,
	UiEventDto,
	UiNodeDto,
	UiNodeMetaDto,
	UiParamDto,
	UiSnapshot
} from '../types';

export interface GraphState {
	rootId: NodeId | null;
	nodesById: Map<NodeId, UiNodeDto>;
	childrenById: Map<NodeId, NodeId[]>;
	paramsById: Map<NodeId, UiParamDto>;
	selectedNodeId: NodeId | null;
	lastEventTime?: EventTime;
	requiresResync: boolean;
}

export interface GraphStore extends Readable<GraphState> {
	loadSnapshot(snapshot: UiSnapshot): void;
	applyEvent(event: UiEventDto): void;
	applyBatch(batch: UiEventBatch): void;
	selectNode(nodeId: NodeId | null): void;
	resetSelection(): void;
}

const createEmptyState = (): GraphState => ({
	rootId: null,
	nodesById: new Map(),
	childrenById: new Map(),
	paramsById: new Map(),
	selectedNodeId: null,
	lastEventTime: undefined,
	requiresResync: false
});

const detectRoot = (nodesById: Map<NodeId, UiNodeDto>, childrenById: Map<NodeId, NodeId[]>): NodeId | null => {
	const childSet = new Set<NodeId>();
	for (const children of childrenById.values()) {
		for (const child of children) {
			childSet.add(child);
		}
	}

	for (const nodeId of nodesById.keys()) {
		if (!childSet.has(nodeId)) {
			return nodeId;
		}
	}

	return null;
};

const stateFromSnapshot = (snapshot: UiSnapshot): GraphState => {
	const nodesById = new Map<NodeId, UiNodeDto>();
	const childrenById = new Map<NodeId, NodeId[]>();
	const paramsById = new Map<NodeId, UiParamDto>();

	for (const node of snapshot.nodes) {
		nodesById.set(node.node_id, node);
		childrenById.set(node.node_id, [...node.children]);
		if (node.data.kind === 'parameter') {
			paramsById.set(node.node_id, node.data.param);
		}
	}

	return {
		rootId: detectRoot(nodesById, childrenById),
		nodesById,
		childrenById,
		paramsById,
		selectedNodeId: null,
		lastEventTime: snapshot.at,
		requiresResync: false
	};
};

const removeFromChildren = (childrenById: Map<NodeId, NodeId[]>, parent: NodeId, child: NodeId): void => {
	const existing = childrenById.get(parent);
	if (!existing) {
		return;
	}
	childrenById.set(
		parent,
		existing.filter((entry) => entry !== child)
	);
};

const addToChildren = (childrenById: Map<NodeId, NodeId[]>, parent: NodeId, child: NodeId): void => {
	const existing = childrenById.get(parent) ?? [];
	if (existing.includes(child)) {
		return;
	}
	childrenById.set(parent, [...existing, child]);
};

const replaceInChildren = (
	childrenById: Map<NodeId, NodeId[]>,
	parent: NodeId,
	oldChild: NodeId,
	newChild: NodeId
): void => {
	const existing = childrenById.get(parent);
	if (!existing) {
		return;
	}
	childrenById.set(
		parent,
		existing.map((entry) => (entry === oldChild ? newChild : entry))
	);
};

const removeSubtree = (state: GraphState, nodeId: NodeId): void => {
	const children = state.childrenById.get(nodeId) ?? [];
	for (const child of children) {
		removeSubtree(state, child);
	}
	state.childrenById.delete(nodeId);
	state.nodesById.delete(nodeId);
	state.paramsById.delete(nodeId);
	if (state.selectedNodeId === nodeId) {
		state.selectedNodeId = null;
	}
};

const applyMetaPatch = (node: UiNodeDto, patch: Partial<UiNodeMetaDto>): UiNodeDto => ({
	...node,
	meta: {
		...node.meta,
		...patch
	}
});

const reduceEvent = (state: GraphState, event: UiEventDto): GraphState => {
	const next: GraphState = {
		...state,
		nodesById: new Map(state.nodesById),
		childrenById: new Map(state.childrenById),
		paramsById: new Map(state.paramsById),
		lastEventTime: event.time
	};

	switch (event.kind.kind) {
		case 'paramChanged': {
			const node = next.nodesById.get(event.kind.param);
			if (!node || node.data.kind !== 'parameter') {
				next.requiresResync = true;
				break;
			}
			const updatedParam = {
				...node.data.param,
				value: event.kind.new_value
			};
			next.paramsById.set(event.kind.param, updatedParam);
			next.nodesById.set(event.kind.param, {
				...node,
				data: { kind: 'parameter', param: updatedParam }
			});
			break;
		}
		case 'childAdded': {
			addToChildren(next.childrenById, event.kind.parent, event.kind.child);
			if (!next.nodesById.has(event.kind.child)) {
				next.requiresResync = true;
			}
			break;
		}
		case 'childRemoved': {
			removeFromChildren(next.childrenById, event.kind.parent, event.kind.child);
			if (next.nodesById.has(event.kind.child)) {
				removeSubtree(next, event.kind.child);
			}
			break;
		}
		case 'childReplaced': {
			replaceInChildren(next.childrenById, event.kind.parent, event.kind.old, event.kind.new);
			if (next.nodesById.has(event.kind.old)) {
				removeSubtree(next, event.kind.old);
			}
			if (!next.nodesById.has(event.kind.new)) {
				next.requiresResync = true;
			}
			break;
		}
		case 'childMoved': {
			removeFromChildren(next.childrenById, event.kind.old_parent, event.kind.child);
			addToChildren(next.childrenById, event.kind.new_parent, event.kind.child);
			break;
		}
		case 'childReordered': {
			// No index payload yet, so order cannot be reconstructed reliably.
			next.requiresResync = true;
			break;
		}
		case 'nodeCreated': {
			if (!next.nodesById.has(event.kind.node)) {
				next.requiresResync = true;
			}
			break;
		}
		case 'nodeDeleted': {
			if (next.nodesById.has(event.kind.node)) {
				removeSubtree(next, event.kind.node);
			}
			break;
		}
		case 'metaChanged': {
			const node = next.nodesById.get(event.kind.node);
			if (!node) {
				next.requiresResync = true;
				break;
			}
			next.nodesById.set(event.kind.node, applyMetaPatch(node, event.kind.patch));
			break;
		}
		case 'custom': {
			break;
		}
	}

	next.rootId = detectRoot(next.nodesById, next.childrenById);
	return next;
};

export const createGraphStore = (): GraphStore => {
	const { subscribe, set, update } = writable<GraphState>(createEmptyState());

	return {
		subscribe,
		loadSnapshot(snapshot: UiSnapshot): void {
			set(stateFromSnapshot(snapshot));
		},
		applyEvent(event: UiEventDto): void {
			update((state) => reduceEvent(state, event));
		},
		applyBatch(batch: UiEventBatch): void {
			update((state) => {
				let next = state;
				for (const event of batch.events) {
					next = reduceEvent(next, event);
				}
				if (batch.to) {
					next = { ...next, lastEventTime: batch.to };
				}
				return next;
			});
		},
		selectNode(nodeId: NodeId | null): void {
			update((state) => ({
				...state,
				selectedNodeId: nodeId
			}));
		},
		resetSelection(): void {
			update((state) => ({
				...state,
				selectedNodeId: null
			}));
		}
	};
};
