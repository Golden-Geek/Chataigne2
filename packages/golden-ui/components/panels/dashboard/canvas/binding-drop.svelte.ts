import type { GraphState } from '../../../../store/graph.svelte';
import { resolveDragNode, resolveDragParameterNode } from '../../../common/node-drag';
import { type UiNodeDto } from '../../../../types';
import {
	bindDashboardGenericWidgetTarget,
	bindDashboardNodeWidgetTarget
} from '../dashboard-actions';
import {
	readNodeDragPayload,
	type NodeDragPayload,
	type NodeDragScope
} from '../../../common/node-drag';
interface DashboardBindingDropContext {
	readonly session: NodeDragScope | null;
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly editMode: boolean;
}
export function createDashboardBindingDrop(context: DashboardBindingDropContext) {
	let liveNode = $derived(context.node);
	let editMode = $derived(context.editMode);
	const getGraph = () => context.graph;
	const resolveDraggedNode = (payload: NodeDragPayload | null) =>
		resolveDragNode(context.graph, payload, context.session);
	const resolveDraggedParameterNode = (payload: NodeDragPayload | null) =>
		resolveDragParameterNode(context.graph, payload, context.session);
	let bindingDragDepth = $state(0);

	const canBindNodeWidget = (payload: NodeDragPayload | null): boolean => {
		return editMode && resolveDraggedNode(payload) !== null;
	};

	const canBindGenericWidget = (payload: NodeDragPayload | null): boolean => {
		return editMode && resolveDraggedParameterNode(payload) !== null;
	};

	const handleBindingDragEnter = (
		event: DragEvent,
		predicate: (payload: NodeDragPayload | null) => boolean
	): void => {
		const payload = readNodeDragPayload(event);
		if (!predicate(payload)) {
			return;
		}
		event.preventDefault();
		bindingDragDepth += 1;
	};

	const handleBindingDragOver = (
		event: DragEvent,
		predicate: (payload: NodeDragPayload | null) => boolean
	): void => {
		const payload = readNodeDragPayload(event);
		if (!predicate(payload)) {
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'copy';
		}
	};

	const handleBindingDragLeave = (): void => {
		bindingDragDepth = Math.max(0, bindingDragDepth - 1);
	};

	const handleNodeWidgetBindingDrop = async (event: DragEvent): Promise<void> => {
		const payload = readNodeDragPayload(event);
		bindingDragDepth = 0;
		if (!canBindNodeWidget(payload)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const targetNode = resolveDraggedNode(payload);
		if (!targetNode) {
			return;
		}
		await bindDashboardNodeWidgetTarget(getGraph, liveNode.node_id, targetNode);
	};

	const handleGenericWidgetBindingDrop = async (event: DragEvent): Promise<void> => {
		const payload = readNodeDragPayload(event);
		bindingDragDepth = 0;
		if (!canBindGenericWidget(payload)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		const targetNode = resolveDraggedParameterNode(payload);
		if (!targetNode) {
			return;
		}
		await bindDashboardGenericWidgetTarget(getGraph, liveNode.node_id, targetNode);
	};

	return {
		get bindingDragDepth() {
			return bindingDragDepth;
		},
		canBindNodeWidget,
		canBindGenericWidget,
		handleBindingDragEnter,
		handleBindingDragOver,
		handleBindingDragLeave,
		handleNodeWidgetBindingDrop,
		handleGenericWidgetBindingDrop
	};
}
