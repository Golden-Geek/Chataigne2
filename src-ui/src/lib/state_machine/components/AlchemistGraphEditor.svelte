<script lang="ts">
	import {
		GraphCanvas,
		type GraphConnectionRequest,
		type GraphNode,
		type GraphNodeCreationRequest,
		type GraphNodeMove,
		type GraphNodePosition,
		type GraphNodeResize
	} from 'golden_alchemist_ui';
	import type { NodeId, UiCreatableUserItem, UiNodeDto } from 'golden_ui';
	import { configParameters, toGraphEdges, toGraphNodes } from '../alchemistGraph';
	import ANodeConfigEditor from './ANodeConfigEditor.svelte';

	let {
		formula,
		nodesById,
		selectedNodeIds = [],
		selectedEdgeIds = [],
		catalogItems = [],
		onGraphSelectionChange,
		onNodesMove,
		onNodeResize,
		onConnect,
		onBackgroundContextMenu,
		onCreateRequest
	}: {
		formula: UiNodeDto;
		nodesById: ReadonlyMap<NodeId, UiNodeDto>;
		selectedNodeIds?: string[];
		selectedEdgeIds?: string[];
		catalogItems?: UiCreatableUserItem[];
		onGraphSelectionChange?: (nodeIds: string[], edgeIds: string[]) => void;
		onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
		onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
		onConnect?: (connection: GraphConnectionRequest) => void;
		onBackgroundContextMenu?: (event: MouseEvent, position: GraphNodePosition) => void;
		onCreateRequest?: (request: GraphNodeCreationRequest) => void;
	} = $props();

	let nodes = $derived(toGraphNodes(formula, nodesById, catalogItems));
	let edges = $derived(toGraphEdges(formula, nodesById));
	let graphCanvas: {
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
		viewportCenter: () => GraphNodePosition;
	} | null = $state(null);

	const authoredNode = (id: string): UiNodeDto | null => nodesById.get(Number(id)) ?? null;

	export const frameSelection = (): boolean => graphCanvas?.frameSelection() ?? false;
	export const home = (): boolean => graphCanvas?.home() ?? false;
	export const focus = (): void => graphCanvas?.focus();
	export const viewportCenter = (): GraphNodePosition =>
		graphCanvas?.viewportCenter() ?? { x: 0, y: 0 };
</script>

{#snippet nodeContent(graphNode: GraphNode)}
	{@const node = authoredNode(graphNode.id)}
	{#if node}
		<ANodeConfigEditor parameters={configParameters(node, nodesById)} />
	{/if}
{/snippet}

<div class="alchemist-graph-editor">
	<GraphCanvas
		bind:this={graphCanvas}
		{nodes}
		{edges}
		{selectedNodeIds}
		{selectedEdgeIds}
		{onGraphSelectionChange}
		{onNodesMove}
		{onNodeResize}
		{onConnect}
		{onBackgroundContextMenu}
		{onCreateRequest}
		{nodeContent}
		routeEdgesAroundNodes={true}
		socketLabels="always"
		emptyLabel="Add an ANode to start this Formula." />
</div>

<style>
	.alchemist-graph-editor {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}
</style>
