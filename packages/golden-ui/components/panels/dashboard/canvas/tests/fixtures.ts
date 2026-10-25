import { VersionedNodeMap } from '../../../../../store/graph-index';
import type { GraphState } from '../../../../../store/graph.svelte';
import type { ParamValue, UiNodeDto, UiParamDto } from '../../../../../types';

export function node(id: number, type: string, children: number[] = []): UiNodeDto {
	return {
		node_id: id,
		uuid: `node-${id}`,
		decl_id: '',
		node_type: type,
		meta: {
			short_name: String(id),
			label: String(id),
			enabled: true,
			can_be_disabled: false,
			tags: [],
			user_permissions: {
				can_edit_name: true,
				can_remove_and_duplicate: true,
				can_edit_constraints: true,
				can_edit_tags: true,
				can_edit_color: true
			}
		},
		data: { kind: 'node', node_type: type },
		user_role: 'regular',
		user_item_kind: '',
		accepted_user_item_kinds: [],
		creatable_user_items: [],
		children
	};
}

export function param(id: number, key: string, value: ParamValue): UiNodeDto {
	const result = node(id, 'parameter');
	result.decl_id = key;
	result.data = {
		kind: 'parameter',
		param: {
			value,
			default_value: value,
			event_behaviour: 'Coalesce',
			read_only: false,
			constraints: { enum_options: [], policy: 'ClampAdapt' },
			ui_hints: {},
			control: { mode: 'manual', spec: { mode: 'manual' } },
			reference_allowed_targets: [],
			reference_visible_nodes: []
		}
	};
	return result;
}

export function graphFixture(count: number): GraphState {
	const nodes = [node(0, 'root', [1]), node(1, 'dashboard_page')];
	for (let index = 0; index < count; index++) {
		const id = 10 + index * 4;
		nodes[1].children.push(id);
		const widget = node(id, 'dashboard_generic_widget', [id + 1, id + 2, id + 3]);
		widget.user_role = 'itemRoot';
		nodes.push(
			widget,
			param(id + 1, 'position', { kind: 'vec2', value: [index * 256, 0] }),
			param(id + 2, 'width', { kind: 'css_value', value: 12, unit: 'rem' }),
			param(id + 3, 'height', { kind: 'css_value', value: 4, unit: 'rem' })
		);
	}
	const graph: GraphState = {
		rootId: 0,
		nodesById: new VersionedNodeMap<UiNodeDto>(),
		childrenById: new VersionedNodeMap<number[]>(),
		parentById: new VersionedNodeMap<number>(),
		paramsById: new VersionedNodeMap<UiParamDto>(),
		requiresResync: false
	};
	for (const item of nodes) {
		graph.nodesById.set(item.node_id, item);
		graph.childrenById.set(item.node_id, item.children);
		for (const child of item.children) graph.parentById.set(child, item.node_id);
		if (item.data.kind === 'parameter') graph.paramsById.set(item.node_id, item.data.param);
	}
	return graph;
}

export const forkGraph = (graph: GraphState): GraphState => ({
	...graph,
	nodesById: graph.nodesById.fork(),
	childrenById: graph.childrenById.fork(),
	parentById: graph.parentById.fork(),
	paramsById: graph.paramsById.fork()
});

export function changeParam(graph: GraphState, id: number, value: ParamValue): GraphState {
	const next = forkGraph(graph);
	const current = graph.nodesById.get(id)!;
	if (current.data.kind !== 'parameter') throw new Error('Expected parameter');
	const updated = { ...current.data.param, value };
	next.nodesById.set(id, { ...current, data: { kind: 'parameter', param: updated } });
	next.paramsById.set(id, updated);
	return next;
}
