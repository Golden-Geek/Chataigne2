import type { UiEventDto, UiGraphOp, UiNodeDto, UiStagedEventWork } from '../types';
import type { GraphState } from './graph.svelte';

type SubtreeInsertedOp = Extract<UiGraphOp, { kind: 'subtreeInserted' }>;
type SubtreeRemovedOp = Extract<UiGraphOp, { kind: 'subtreeRemoved' }>;
type ProjectedGraphOp = SubtreeInsertedOp | SubtreeRemovedOp;

export interface GraphEventProjectionResult {
	workUsed: number;
	done: boolean;
}

export interface GraphEventProjectionWork extends UiStagedEventWork {
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

const isProjectableTransaction = (
	event: UiEventDto
): event is UiEventDto & {
	kind: Extract<UiEventDto['kind'], { kind: 'graphTransaction' }>;
} =>
	event.kind.kind === 'graphTransaction' &&
	event.kind.ops.length > 0 &&
	event.kind.ops.every((op) => op.kind === 'subtreeInserted' || op.kind === 'subtreeRemoved');

export const canProjectGraphEventIncrementally = (event: UiEventDto): boolean =>
	isProjectableTransaction(event);

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
			if (!task.nextChildren.has(child) && state.parentById.get(child) === task.node.node_id) {
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
	parent: number,
	sourceChildren: number[]
): ParentProjectionTask => ({
	parent,
	children: [],
	nextChildren: new Set(),
	sourceChildren,
	previousChildren: state.childrenById.get(parent) ?? [],
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
	if (!isProjectableTransaction(options.event)) {
		return undefined;
	}

	const baseState = options.baseState;
	const nextState: GraphState = {
		rootId: baseState.rootId,
		nodesById: baseState.nodesById.fork(),
		childrenById: baseState.childrenById.fork(),
		parentById: baseState.parentById.fork(),
		paramsById: baseState.paramsById.fork(),
		lastEventTime: options.event.time,
		requiresResync: baseState.requiresResync
	};
	const ops = options.event.kind.ops as ProjectedGraphOp[];
	let done = false;
	let opIndex = 0;
	let nodeIndex = 0;
	let removedIdIndex = 0;
	let nodeTask: NodeProjectionTask | undefined;
	let parentTask: ParentProjectionTask | undefined;
	let prepared = false;
	let cancelled = false;

	const advanceOneOpStep = (): boolean => {
		const op = ops[opIndex];
		if (!op) {
			done = true;
			return false;
		}
		if (op.kind === 'subtreeRemoved') {
			const removedId = op.removed_ids[removedIdIndex];
			if (removedId !== undefined) {
				nextState.childrenById.delete(removedId);
				nextState.parentById.delete(removedId);
				nextState.nodesById.delete(removedId);
				nextState.paramsById.delete(removedId);
				if (nextState.rootId === removedId) {
					nextState.rootId = null;
				}
				removedIdIndex += 1;
				return true;
			}
			if (!parentTask && op.parent_after) {
				parentTask = createParentTask(nextState, op.parent_after.parent, op.parent_after.children);
				return true;
			}
			if (parentTask) {
				if (advanceParentTask(nextState, parentTask)) {
					parentTask = undefined;
					removedIdIndex = 0;
					opIndex += 1;
				}
				return true;
			}
			removedIdIndex = 0;
			opIndex += 1;
			return true;
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
			parentTask = createParentTask(nextState, op.parent, op.parent_children_after);
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
			while (workUsed < budget && !done) {
				if (advanceOneOpStep()) {
					workUsed += 1;
				}
			}
			if (done) {
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
