import type {
	EventTime,
	NodeId,
	ParamValue,
	UiAck,
	UiClient,
	UiEditIntent,
	UiEventBatch,
	UiEventDto,
	UiNodeDto,
	UiParamConstraints,
	UiSnapshot,
	UiSubscriptionScope
} from '../types';
import { wholeGraphScope } from '../types';

interface MockGraph {
	nodes: Map<NodeId, UiNodeDto>;
	rootId: NodeId;
	time: EventTime;
	eventLog: UiEventDto[];
	activeEditId?: string;
}

interface Subscriber {
	scope: UiSubscriptionScope;
	onBatch: (batch: UiEventBatch) => void;
}

const compareEventTime = (left: EventTime, right: EventTime): number => {
	if (left.tick !== right.tick) {
		return left.tick - right.tick;
	}
	if (left.micro !== right.micro) {
		return left.micro - right.micro;
	}
	return left.seq - right.seq;
};

const eventAfter = (event: EventTime, cursor?: EventTime): boolean => {
	if (!cursor) {
		return true;
	}
	return compareEventTime(event, cursor) > 0;
};

const buildParentIndex = (nodes: Map<NodeId, UiNodeDto>): Map<NodeId, NodeId> => {
	const parents = new Map<NodeId, NodeId>();
	for (const node of nodes.values()) {
		for (const child of node.children) {
			parents.set(child, node.node_id);
		}
	}
	return parents;
};

const nodeInScope = (
	scope: UiSubscriptionScope,
	nodeId: NodeId,
	nodes: Map<NodeId, UiNodeDto>,
	parents: Map<NodeId, NodeId>
): boolean => {
	if (scope.kind === 'wholeGraph') {
		return nodes.has(nodeId);
	}
	let current: NodeId | undefined = nodeId;
	let depth = 0;
	while (current !== undefined && depth <= scope.max_depth) {
		if (current === scope.root) {
			return true;
		}
		current = parents.get(current);
		depth += 1;
	}
	return false;
};

const eventInScope = (
	scope: UiSubscriptionScope,
	event: UiEventDto,
	nodes: Map<NodeId, UiNodeDto>,
	parents: Map<NodeId, NodeId>
): boolean => {
	if (scope.kind === 'wholeGraph') {
		return true;
	}

	const candidates: NodeId[] = (() => {
		switch (event.kind.kind) {
			case 'paramChanged':
				return [event.kind.param];
			case 'childAdded':
				return [event.kind.parent, event.kind.child];
			case 'childRemoved':
				return [event.kind.parent, event.kind.child];
			case 'childReplaced':
				return [event.kind.parent, event.kind.old, event.kind.new];
			case 'childMoved':
				return [event.kind.child, event.kind.old_parent, event.kind.new_parent];
			case 'childReordered':
				return [event.kind.parent, event.kind.child];
			case 'nodeCreated':
				return [event.kind.node];
			case 'nodeDeleted':
				return [event.kind.node];
			case 'metaChanged':
				return [event.kind.node];
			case 'custom':
				return event.kind.origin !== undefined ? [event.kind.origin] : [];
		}
	})();

	return candidates.some((nodeId) => nodeInScope(scope, nodeId, nodes, parents));
};

const cloneNode = (node: UiNodeDto): UiNodeDto => ({
	...node,
	meta: { ...node.meta, tags: [...node.meta.tags] },
	data:
		node.data.kind === 'parameter'
			? {
					kind: 'parameter',
					param: {
						...node.data.param,
						constraints: {
							...node.data.param.constraints,
							enum_options: node.data.param.constraints.enum_options.map((option) => ({
								...option,
								value: structuredClone(option.value),
								tags: [...option.tags]
							}))
						},
						ui_hints: { ...node.data.param.ui_hints },
						value: structuredClone(node.data.param.value)
					}
				}
			: { ...node.data },
	children: [...node.children]
});

const scopeNodeIds = (scope: UiSubscriptionScope, graph: MockGraph): NodeId[] => {
	if (scope.kind === 'wholeGraph') {
		return [...graph.nodes.keys()];
	}
	const out: NodeId[] = [];
	const stack: Array<{ nodeId: NodeId; depth: number }> = [{ nodeId: scope.root, depth: 0 }];
	while (stack.length > 0) {
		const current = stack.pop();
		if (!current) {
			continue;
		}
		const node = graph.nodes.get(current.nodeId);
		if (!node) {
			continue;
		}
		out.push(current.nodeId);
		if (current.depth >= scope.max_depth) {
			continue;
		}
		for (let index = node.children.length - 1; index >= 0; index -= 1) {
			stack.push({ nodeId: node.children[index], depth: current.depth + 1 });
		}
	}
	return out;
};

const snapshotForScope = (graph: MockGraph, scope: UiSubscriptionScope): UiSnapshot => {
	const ids = scopeNodeIds(scope, graph);
	const visible = new Set(ids);
	const nodes = ids
		.map((id) => graph.nodes.get(id))
		.filter((node): node is UiNodeDto => node !== undefined)
		.map((node) => {
			const cloned = cloneNode(node);
			cloned.children = cloned.children.filter((child) => visible.has(child));
			return cloned;
		});

	return {
		protocol_version: '0.1.0',
		scope,
		at: graph.time,
		nodes,
		schema: {
			node_types: [...new Set(nodes.map((node) => node.node_type))].sort().map((node_type) => ({ node_type })),
			enums: []
		}
	};
};

const replayFrom = (graph: MockGraph, scope: UiSubscriptionScope, from?: EventTime): UiEventBatch => {
	const parents = buildParentIndex(graph.nodes);
	const events = graph.eventLog.filter(
		(event) => eventAfter(event.time, from) && eventInScope(scope, event, graph.nodes, parents)
	);
	return {
		from,
		to: events.length > 0 ? events[events.length - 1].time : undefined,
		events
	};
};

const createInitialGraph = (): MockGraph => {
	const nodes = new Map<NodeId, UiNodeDto>();

	nodes.set(1, {
		node_id: 1,
		uuid: '00000000-0000-0000-0000-000000000001',
		decl_id: 'root',
		node_type: 'manager',
		meta: {
			short_name: 'root',
			label: 'Root',
			enabled: true,
			can_be_disabled: false,
			tags: ['project']
		},
		data: { kind: 'node', node_type: 'manager' },
		children: [2, 3]
	});

	nodes.set(2, {
		node_id: 2,
		uuid: '00000000-0000-0000-0000-000000000002',
		decl_id: 'intensity',
		node_type: 'float',
		meta: {
			short_name: 'intensity',
			label: 'Intensity',
			enabled: true,
			can_be_disabled: true,
			description: 'Primary modulation amount',
			tags: ['parameter']
		},
		data: {
			kind: 'parameter',
			param: {
				value: { kind: 'float', value: 0.65 },
				event_behaviour: 'Coalesce',
				read_only: false,
				constraints: {
					min: 0,
					max: 1,
					step: 0.01,
					step_base: 0,
					enum_options: [],
					policy: 'ClampAdapt'
				},
				ui_hints: { widget: 'slider' }
			}
		},
		children: []
	});

	nodes.set(3, {
		node_id: 3,
		uuid: '00000000-0000-0000-0000-000000000003',
		decl_id: 'osc',
		node_type: 'oscOutput',
		meta: {
			short_name: 'osc',
			label: 'OSC Output',
			enabled: true,
			can_be_disabled: true,
			tags: ['module']
		},
		data: { kind: 'node', node_type: 'oscOutput' },
		children: [4]
	});

	nodes.set(4, {
		node_id: 4,
		uuid: '00000000-0000-0000-0000-000000000004',
		decl_id: 'enabled',
		node_type: 'bool',
		meta: {
			short_name: 'enabled',
			label: 'Enabled',
			enabled: true,
			can_be_disabled: true,
			tags: ['parameter']
		},
		data: {
			kind: 'parameter',
			param: {
				value: { kind: 'bool', value: true },
				event_behaviour: 'Coalesce',
				read_only: false,
				constraints: {
					enum_options: [],
					policy: 'ClampAdapt'
				},
				ui_hints: { widget: 'toggle' }
			}
		},
		children: []
	});

	return {
		nodes,
		rootId: 1,
		time: { tick: 0, micro: 0, seq: 0 },
		eventLog: []
	};
};

const nextEventTime = (graph: MockGraph): EventTime => {
	graph.time = {
		...graph.time,
		seq: graph.time.seq + 1
	};
	return graph.time;
};

const ackApplied = (earliest_event_time?: EventTime): UiAck => ({
	success: true,
	status: 'applied',
	earliest_event_time
});

const ackRejected = (error_code: string, error_message: string): UiAck => ({
	success: false,
	status: 'rejected',
	error_code,
	error_message
});

const paramValueEquals = (left: ParamValue, right: ParamValue): boolean =>
	JSON.stringify(left) === JSON.stringify(right);

const normalizeParamValue = (
	value: ParamValue,
	constraints: UiParamConstraints
): { ok: true; value: ParamValue } | { ok: false; error: string } => {
	const policy = constraints.policy ?? 'ClampAdapt';
	const min = constraints.min;
	const max = constraints.max;
	const step = constraints.step;
	const stepBase = constraints.step_base ?? min ?? 0;

	const normalizeNumeric = (input: number, wantsInt: boolean): { ok: true; value: number } | { ok: false; error: string } => {
		let output = input;

		if (min !== undefined && max !== undefined && min > max) {
			return { ok: false, error: `invalid constraints: min ${min} is greater than max ${max}` };
		}

		if (min !== undefined && output < min) {
			if (policy === 'ClampAdapt') {
				output = min;
			} else {
				return { ok: false, error: `value ${output} is lower than min ${min}` };
			}
		}

		if (max !== undefined && output > max) {
			if (policy === 'ClampAdapt') {
				output = max;
			} else {
				return { ok: false, error: `value ${output} is higher than max ${max}` };
			}
		}

		if (step !== undefined) {
			if (step <= 0) {
				return { ok: false, error: `invalid step ${step}: expected positive value` };
			}
			const scaled = (output - stepBase) / step;
			const nearest = Math.round(scaled);
			if (policy === 'ClampAdapt') {
				output = stepBase + nearest * step;
			} else if (Math.abs(scaled - nearest) > 1e-9) {
				return {
					ok: false,
					error: `value ${output} does not align with step ${step} from base ${stepBase}`
				};
			}
		}

		if (policy === 'ClampAdapt') {
			if (min !== undefined) {
				output = Math.max(output, min);
			}
			if (max !== undefined) {
				output = Math.min(output, max);
			}
		}

		if (wantsInt) {
			const rounded = Math.round(output);
			if (policy === 'Reject' && Math.abs(output - rounded) > 1e-9) {
				return { ok: false, error: `value ${output} is not an integer` };
			}
			output = rounded;
		}

		return { ok: true, value: output };
	};

	let normalized = value;
	if (value.kind === 'int') {
		const normalizedNumeric = normalizeNumeric(value.value, true);
		if (!normalizedNumeric.ok) {
			return normalizedNumeric;
		}
		normalized = { kind: 'int', value: normalizedNumeric.value };
	} else if (value.kind === 'float') {
		const normalizedNumeric = normalizeNumeric(value.value, false);
		if (!normalizedNumeric.ok) {
			return normalizedNumeric;
		}
		normalized = { kind: 'float', value: normalizedNumeric.value };
	}

	if (constraints.enum_options.length > 0) {
		const allowed = constraints.enum_options.some((option) => paramValueEquals(option.value, normalized));
		if (!allowed) {
			return {
				ok: false,
				error: `value is not in enum options: allowed variants ${constraints.enum_options.map((option) => option.variant_id).join(', ')}`
			};
		}
	}

	return { ok: true, value: normalized };
};

export const createMockUiClient = (): UiClient => {
	const graph = createInitialGraph();
	const subscribers: Subscriber[] = [];

	const publish = (event: UiEventDto): void => {
		graph.eventLog.push(event);
		const parents = buildParentIndex(graph.nodes);
		for (const subscriber of subscribers) {
			if (!eventInScope(subscriber.scope, event, graph.nodes, parents)) {
				continue;
			}
			subscriber.onBatch({
				from: undefined,
				to: event.time,
				events: [event]
			});
		}
	};

	return {
		async snapshot(scope = wholeGraphScope): Promise<UiSnapshot> {
			return snapshotForScope(graph, scope);
		},
		subscribe(
			scope: UiSubscriptionScope,
			from: EventTime | undefined,
			onBatch: (batch: UiEventBatch) => void
		): () => void {
			if (from) {
				onBatch(replayFrom(graph, scope, from));
			}
			const subscriber: Subscriber = { scope, onBatch };
			subscribers.push(subscriber);
			return () => {
				const index = subscribers.indexOf(subscriber);
				if (index >= 0) {
					subscribers.splice(index, 1);
				}
			};
		},
		async replay(scope: UiSubscriptionScope, from?: EventTime): Promise<UiEventBatch> {
			return replayFrom(graph, scope, from);
		},
		async sendIntent(intent: UiEditIntent): Promise<UiAck> {
			switch (intent.kind) {
				case 'beginEdit': {
					if (graph.activeEditId !== undefined) {
						return ackRejected('edit_session_already_active', 'An edit session is already active.');
					}
					graph.activeEditId = intent.client_edit_id;
					return ackApplied();
				}
				case 'endEdit': {
					if (graph.activeEditId === undefined) {
						return ackRejected('edit_session_not_active', 'No edit session is active.');
					}
					if (graph.activeEditId !== intent.client_edit_id) {
						return ackRejected('edit_session_id_mismatch', 'The provided edit session id does not match the active session.');
					}
					graph.activeEditId = undefined;
					return ackApplied();
				}
				case 'setParam': {
					const node = graph.nodes.get(intent.node);
					if (!node || node.data.kind !== 'parameter') {
						return ackRejected('node_not_found', 'Parameter node was not found.');
					}
					if (node.data.param.read_only) {
						return ackRejected('read_only', 'Parameter is read-only.');
					}
					const normalized = normalizeParamValue(intent.value, node.data.param.constraints);
					if (!normalized.ok) {
						return ackRejected('param_constraint_violation', normalized.error);
					}
					const oldValue = node.data.param.value;
					const newParam = {
						...node.data.param,
						value: normalized.value,
						event_behaviour: intent.behaviour
					};
					graph.nodes.set(intent.node, {
						...node,
						data: { kind: 'parameter', param: newParam }
					});
					const time = nextEventTime(graph);
					publish({
						time,
						kind: {
							kind: 'paramChanged',
							param: intent.node,
							old_value: oldValue,
							new_value: normalized.value
						}
					});
					return ackApplied(time);
				}
				case 'patchMeta': {
					const node = graph.nodes.get(intent.node);
					if (!node) {
						return ackRejected('node_not_found', 'Node was not found.');
					}
					graph.nodes.set(intent.node, {
						...node,
						meta: {
							...node.meta,
							...intent.patch
						}
					});
					const time = nextEventTime(graph);
					publish({
						time,
						kind: {
							kind: 'metaChanged',
							node: intent.node,
							patch: intent.patch
						}
					});
					return ackApplied(time);
				}
				case 'moveNode': {
					const target = graph.nodes.get(intent.node);
					const newParent = graph.nodes.get(intent.new_parent);
					if (!target || !newParent) {
						return ackRejected('node_not_found', 'Node or destination parent was not found.');
					}
					const parents = buildParentIndex(graph.nodes);
					const oldParentId = parents.get(intent.node);
					if (oldParentId === undefined) {
						return ackRejected('cannot_mutate_root', 'Root node cannot be moved.');
					}
					const oldParent = graph.nodes.get(oldParentId);
					if (!oldParent) {
						return ackRejected('node_not_found', 'Source parent node was not found.');
					}
					graph.nodes.set(oldParentId, {
						...oldParent,
						children: oldParent.children.filter((child) => child !== intent.node)
					});
					graph.nodes.set(intent.new_parent, {
						...newParent,
						children: [...newParent.children, intent.node]
					});
					const time = nextEventTime(graph);
					publish({
						time,
						kind: {
							kind: 'childMoved',
							child: intent.node,
							old_parent: oldParentId,
							new_parent: intent.new_parent
						}
					});
					return ackApplied(time);
				}
				case 'removeNode': {
					const target = graph.nodes.get(intent.node);
					if (!target) {
						return ackRejected('node_not_found', 'Node was not found.');
					}
					const parents = buildParentIndex(graph.nodes);
					const parentId = parents.get(intent.node);
					if (parentId === undefined) {
						return ackRejected('cannot_mutate_root', 'Root node cannot be removed.');
					}

					const removeSubtree = (nodeId: NodeId): void => {
						const node = graph.nodes.get(nodeId);
						if (!node) {
							return;
						}
						for (const child of node.children) {
							removeSubtree(child);
						}
						graph.nodes.delete(nodeId);
						const deleteTime = nextEventTime(graph);
						publish({
							time: deleteTime,
							kind: { kind: 'nodeDeleted', node: nodeId }
						});
					};

					removeSubtree(intent.node);
					const parent = graph.nodes.get(parentId);
					if (parent) {
						graph.nodes.set(parentId, {
							...parent,
							children: parent.children.filter((child) => child !== intent.node)
						});
					}
					const time = nextEventTime(graph);
					publish({
						time,
						kind: { kind: 'childRemoved', parent: parentId, child: intent.node }
					});
					return ackApplied(time);
				}
				case 'reevaluateGraph': {
					return ackApplied();
				}
				case 'undo':
				case 'redo': {
					return ackRejected('not_supported', 'Undo/redo is not implemented in the mock transport.');
				}
			}
		}
	};
};
