import type { UiEventDto, UiGraphOp, UiNodeDto } from '../types';
import type { GraphState } from './graph.svelte';

type SubtreeInsertedOp = Extract<UiGraphOp, { kind: 'subtreeInserted' }>;

export interface GraphEventProjectionResult {
	workUsed: number;
	done: boolean;
}

export interface GraphEventProjectionWork {
	advance(maxWork: number): GraphEventProjectionResult;
	cancel(): void;
}

interface NodeProjectionTask {
	node: UiNodeDto;
	children: number[];
	nextChildren: Set<number>;
	previousChildren: number[];
	childIndex: number;
	previousIndex: number;
	phase: 'copyChildren' | 'removePrevious' | 'finalize';
}

interface ParentProjectionTask {
	parent: number;
	children: number[];
	nextChildren: Set<number>;
	sourceChildren: number[];
	previousChildren: number[];
	childIndex: number;
	previousIndex: number;
	phase: 'copyChildren' | 'removePrevious' | 'finalize';
}

interface PreparedGraphProjection {
	baseState: GraphState;
	nextState: GraphState;
}

interface GraphProjectionOptions {
	baseState: GraphState;
	event: UiEventDto;
	onPrepared: (projection: PreparedGraphProjection) => void;
	onCancelled: () => void;
}

const isSubtreeOnlyTransaction = (
	event: UiEventDto
): event is UiEventDto & {
	kind: Extract<UiEventDto['kind'], { kind: 'graphTransaction' }>;
} =>
	event.kind.kind === 'graphTransaction' &&
	event.kind.ops.length > 0 &&
	event.kind.ops.every((op) => op.kind === 'subtreeInserted');

export const canProjectGraphEventIncrementally = (event: UiEventDto): boolean =>
	isSubtreeOnlyTransaction(event);

const createNodeTask = (state: GraphState, node: UiNodeDto): NodeProjectionTask => ({
	node,
	children: [],
	nextChildren: new Set(),
	previousChildren: state.childrenById.get(node.node_id) ?? [],
	childIndex: 0,
	previousIndex: 0,
	phase: 'copyChildren'
});

const advanceNodeTask = (state: GraphState, task: NodeProjectionTask): boolean => {
	if (task.phase === 'copyChildren') {
		const child = task.node.children[task.childIndex];
		if (child !== undefined) {
			task.children.push(child);
			task.nextChildren.add(child);
			state.parentById.set(child, task.node.node_id);
			task.childIndex += 1;
			return false;
		}
		task.phase = 'removePrevious';
		return false;
	}
	if (task.phase === 'removePrevious') {
		const child = task.previousChildren[task.previousIndex];
		if (child !== undefined) {
			if (
				!task.nextChildren.has(child) &&
				state.parentById.get(child) === task.node.node_id
			) {
				state.parentById.delete(child);
			}
			task.previousIndex += 1;
			return false;
		}
		task.phase = 'finalize';
		return false;
	}

	state.childrenById.set(task.node.node_id, task.children);
	state.nodesById.set(task.node.node_id, {
		...task.node,
		children: task.children
	});
	if (task.node.data.kind === 'parameter') {
		state.paramsById.set(task.node.node_id, task.node.data.param);
	} else {
		state.paramsById.delete(task.node.node_id);
	}
	return true;
};

const createParentTask = (
	state: GraphState,
	op: SubtreeInsertedOp
): ParentProjectionTask => ({
	parent: op.parent,
	children: [],
	nextChildren: new Set(),
	sourceChildren: op.parent_children_after,
	previousChildren: state.childrenById.get(op.parent) ?? [],
	childIndex: 0,
	previousIndex: 0,
	phase: 'copyChildren'
});

const advanceParentTask = (state: GraphState, task: ParentProjectionTask): boolean => {
	if (task.phase === 'copyChildren') {
		const child = task.sourceChildren[task.childIndex];
		if (child !== undefined) {
			task.children.push(child);
			task.nextChildren.add(child);
			state.parentById.set(child, task.parent);
			task.childIndex += 1;
			return false;
		}
		task.phase = 'removePrevious';
		return false;
	}
	if (task.phase === 'removePrevious') {
		const child = task.previousChildren[task.previousIndex];
		if (child !== undefined) {
			if (!task.nextChildren.has(child) && state.parentById.get(child) === task.parent) {
				state.parentById.delete(child);
			}
			task.previousIndex += 1;
			return false;
		}
		task.phase = 'finalize';
		return false;
	}

	state.childrenById.set(task.parent, task.children);
	const parentNode = state.nodesById.get(task.parent);
	if (parentNode) {
		state.nodesById.set(task.parent, {
			...parentNode,
			children: task.children
		});
	} else {
		state.requiresResync = true;
	}
	return true;
};

export const createIncrementalGraphEventProjection = (
	options: GraphProjectionOptions
): GraphEventProjectionWork | undefined => {
	if (!isSubtreeOnlyTransaction(options.event)) {
		return undefined;
	}

	const baseState = options.baseState;
	const nextState: GraphState = {
		rootId: baseState.rootId,
		nodesById: new Map(),
		childrenById: new Map(),
		parentById: new Map(),
		paramsById: new Map(),
		lastEventTime: options.event.time,
		requiresResync: baseState.requiresResync
	};
	const nodeEntries = baseState.nodesById.entries();
	const childrenEntries = baseState.childrenById.entries();
	const parentEntries = baseState.parentById.entries();
	const paramEntries = baseState.paramsById.entries();
	const ops = options.event.kind.ops as SubtreeInsertedOp[];
	let copyPhase: 'nodes' | 'children' | 'parents' | 'params' | 'ops' | 'done' = 'nodes';
	let opIndex = 0;
	let nodeIndex = 0;
	let nodeTask: NodeProjectionTask | undefined;
	let parentTask: ParentProjectionTask | undefined;
	let prepared = false;
	let cancelled = false;

	const copyOneEntry = (): boolean => {
		if (copyPhase === 'nodes') {
			const entry = nodeEntries.next();
			if (!entry.done) {
				nextState.nodesById.set(entry.value[0], entry.value[1]);
				return true;
			}
			copyPhase = 'children';
		}
		if (copyPhase === 'children') {
			const entry = childrenEntries.next();
			if (!entry.done) {
				nextState.childrenById.set(entry.value[0], entry.value[1]);
				return true;
			}
			copyPhase = 'parents';
		}
		if (copyPhase === 'parents') {
			const entry = parentEntries.next();
			if (!entry.done) {
				nextState.parentById.set(entry.value[0], entry.value[1]);
				return true;
			}
			copyPhase = 'params';
		}
		if (copyPhase === 'params') {
			const entry = paramEntries.next();
			if (!entry.done) {
				nextState.paramsById.set(entry.value[0], entry.value[1]);
				return true;
			}
			copyPhase = 'ops';
		}
		return false;
	};

	const advanceOneOpStep = (): boolean => {
		const op = ops[opIndex];
		if (!op) {
			copyPhase = 'done';
			return false;
		}
		if (nodeTask) {
			if (advanceNodeTask(nextState, nodeTask)) {
				nodeTask = undefined;
				nodeIndex += 1;
			}
			return true;
		}
		if (nodeIndex < op.nodes.length) {
			const node = op.nodes[nodeIndex];
			if (node) {
				nodeTask = createNodeTask(nextState, node);
				return true;
			}
			nodeIndex += 1;
			return false;
		}
		if (!parentTask) {
			parentTask = createParentTask(nextState, op);
			return true;
		}
		if (advanceParentTask(nextState, parentTask)) {
			parentTask = undefined;
			nodeIndex = 0;
			opIndex += 1;
		}
		return true;
	};

	const finish = (): void => {
		if (prepared || cancelled) {
			return;
		}
		prepared = true;
		options.onPrepared({ baseState, nextState });
	};

	return {
		advance(maxWork: number): GraphEventProjectionResult {
			if (cancelled || prepared) {
				return { workUsed: 0, done: true };
			}
			const budget = Math.max(1, Math.floor(maxWork));
			let workUsed = 0;
			while (workUsed < budget && copyPhase !== 'done') {
				if (copyPhase !== 'ops') {
					if (copyOneEntry()) {
						workUsed += 1;
					}
					continue;
				}
				if (advanceOneOpStep()) {
					workUsed += 1;
				}
			}
			if (copyPhase === 'done') {
				finish();
			}
			return { workUsed, done: prepared || cancelled };
		},
		cancel(): void {
			if (prepared || cancelled) {
				return;
			}
			cancelled = true;
			options.onCancelled();
		}
	};
};
