import type { GraphState } from '../../../../store/graph.svelte';
import { resolveDashboardDragNode, resolveDashboardDragParameterNode } from '../dashboard-drag';
import { type UiNodeDto } from '../../../../types';
import {
	bindDashboardGenericWidgetTarget,
	bindDashboardNodeWidgetTarget
} from '../dashboard-actions';
import { readDashboardDragPayload, type DashboardDragPayload } from '../dashboard-drag';
interface DashboardBindingDropContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly editMode: boolean;
}
export function createDashboardBindingDrop(context: DashboardBindingDropContext) {
	let liveNode = $derived(context.node);
	let editMode = $derived(context.editMode);
	const getGraph = () => context.graph;
	const resolveDraggedNode = (payload: DashboardDragPayload | null) =>
		resolveDashboardDragNode(context.graph, payload);
	const resolveDraggedParameterNode = (payload: DashboardDragPayload | null) =>
		resolveDashboardDragParameterNode(context.graph, payload);
	let bindingDragDepth = $state(0);

	const canBindNodeWidget = (payload: DashboardDragPayload | null): boolean => {
		return editMode && resolveDraggedNode(payload) !== null;
	};

	const canBindGenericWidget = (payload: DashboardDragPayload | null): boolean => {
		return editMode && resolveDraggedParameterNode(payload) !== null;
	};

	const handleBindingDragEnter = (
		event: DragEvent,
		predicate: (payload: DashboardDragPayload | null) => boolean
	): void => {
		const payload = readDashboardDragPayload(event);
		if (!predicate(payload)) {
			return;
		}
		event.preventDefault();
		bindingDragDepth += 1;
	};

	const handleBindingDragOver = (
		event: DragEvent,
		predicate: (payload: DashboardDragPayload | null) => boolean
	): void => {
		const payload = readDashboardDragPayload(event);
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
		const payload = readDashboardDragPayload(event);
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
		const payload = readDashboardDragPayload(event);
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
