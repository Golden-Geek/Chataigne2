import type { GraphState } from '../../../store/graph.svelte';
import type {
	NodePickerModalOptions,
	NodePickerModalView
} from '../../../store/node-picker-modal.svelte';
import type { UiNodeDto, UiParamValueProjection } from '../../../types';

/** Caller-owned constraints apply equally to initial selection, pointer, keyboard and confirmation. */
export function pickerCandidateAllowed(
	graph: Pick<GraphState, 'nodesById' | 'parentById'> | null,
	options: NodePickerModalOptions | null,
	view: NodePickerModalView | null,
	node: UiNodeDto
): boolean {
	const root = options?.rootNode;
	if (
		!graph ||
		!root ||
		graph.nodesById.get(root.node_id)?.uuid !== root.uuid ||
		graph.nodesById.get(node.node_id)?.uuid !== node.uuid ||
		!(options.nodeFilter?.(node) ?? true) ||
		!(options.nodeVisibilityFilter?.(node) ?? true) ||
		!(view?.nodeVisibilityFilter?.(node) ?? true) ||
		!(view?.nodeRowVisibilityFilter?.(node) ?? true)
	)
		return false;
	const visited = new Set<number>();
	let id: number | undefined = node.node_id;
	while (id !== undefined && !visited.has(id)) {
		if (id === root.node_id) return true;
		visited.add(id);
		id = graph.parentById.get(id);
	}
	return false;
}

export function pickerProjectionAllowed(
	options: NodePickerModalOptions,
	node: UiNodeDto,
	projection: UiParamValueProjection | undefined
): boolean {
	if (!options.projectionOptions) return projection === undefined;
	return projection === undefined
		? !(options.projectionRequired?.(node) ?? false)
		: options.projectionOptions(node).includes(projection);
}
