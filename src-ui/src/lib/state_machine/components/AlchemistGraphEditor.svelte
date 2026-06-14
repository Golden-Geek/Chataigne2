<script lang="ts">
	import { type Snippet } from 'svelte';
	import {
		GraphCanvas,
		type GraphConnectionRequest,
		type GraphNode,
		type GraphNodeCreationRequest,
		type GraphNodeMove,
		type GraphNodePosition,
		type GraphNodeResize,
		type GraphSocket
	} from 'golden_alchemist_ui';
	import type { NodeId, UiCreatableUserItem, UiNodeDto } from 'golden_ui';
	import {
		anodeType,
		bodyConfigParameters,
		canConnectGraphConnection,
		toGraphEdges,
		toGraphNodes,
		valueTypeConfigParameter
	} from '../alchemistGraph';
	import ANodeConfigEditor from './ANodeConfigEditor.svelte';
	import ANodeSocketDefaultEditor from './ANodeSocketDefaultEditor.svelte';
	import ANodeValueTypeHeaderControl from './ANodeValueTypeHeaderControl.svelte';

	let {
		formula,
		nodesById,
		selectedNodeIds = [],
		selectedEdgeIds = [],
		catalogItems = [],
		onGraphSelectionChange,
		onNodesMove,
		onNodeResize,
		onNodeRename,
		onNodeCollapsedChange,
		onConnect,
		onBackgroundContextMenu,
		onCreateRequest,
		toolbarEnd
	}: {
		formula: UiNodeDto;
		nodesById: ReadonlyMap<NodeId, UiNodeDto>;
		selectedNodeIds?: string[];
		selectedEdgeIds?: string[];
		catalogItems?: UiCreatableUserItem[];
		onGraphSelectionChange?: (nodeIds: string[], edgeIds: string[]) => void;
		onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
		onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
		onNodeRename?: (nodeId: string, label: string) => void | Promise<void>;
		onNodeCollapsedChange?: (nodeId: string, collapsed: boolean) => void | Promise<void>;
		onConnect?: (connection: GraphConnectionRequest) => void;
		onBackgroundContextMenu?: (event: MouseEvent, position: GraphNodePosition) => void;
		onCreateRequest?: (request: GraphNodeCreationRequest) => void;
		toolbarEnd?: Snippet;
	} = $props();

	let nodes = $derived(toGraphNodes(formula, nodesById, catalogItems));
	let edges = $derived(toGraphEdges(formula, nodesById));
	let graphCanvas: {
		clientToWorld: (clientX: number, clientY: number) => GraphNodePosition;
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
		viewportCenter: () => GraphNodePosition;
	} | null = $state(null);

	let pendingHome = $state(false);
	let activeFormulaId = $state<NodeId | null>(null);
	const framedFormulaIds = new Set<NodeId>();

	$effect(() => {
		const formulaId = formula.node_id;
		if (formulaId === activeFormulaId) return;
		activeFormulaId = formulaId;
		pendingHome = !framedFormulaIds.has(formulaId);
	});

	$effect(() => {
		if (!pendingHome || activeFormulaId === null) return;
		if (nodes.length === 0) {
			framedFormulaIds.add(activeFormulaId);
			pendingHome = false;
			return;
		}
		if (graphCanvas?.home()) {
			framedFormulaIds.add(activeFormulaId);
			pendingHome = false;
		}
	});

	const authoredNode = (id: string): UiNodeDto | null => nodesById.get(Number(id)) ?? null;
	const isPropertyGetter = (node: UiNodeDto): boolean => anodeType(node) === 'property';
	const inputDefaultParameter = (socket: GraphSocket): UiNodeDto | null =>
		socket.defaultParamId ? (nodesById.get(Number(socket.defaultParamId)) ?? null) : null;
	const canConnect = (connection: GraphConnectionRequest): boolean =>
		canConnectGraphConnection(nodes, connection);

	export const clientToWorld = (clientX: number, clientY: number): GraphNodePosition =>
		graphCanvas?.clientToWorld(clientX, clientY) ?? { x: 0, y: 0 };
	export const frameSelection = (): boolean => graphCanvas?.frameSelection() ?? false;
	export const home = (): boolean => graphCanvas?.home() ?? false;
	export const focus = (): void => graphCanvas?.focus();
	export const viewportCenter = (): GraphNodePosition =>
		graphCanvas?.viewportCenter() ?? { x: 0, y: 0 };
</script>

{#snippet nodeContent(graphNode: GraphNode)}
	{@const node = authoredNode(graphNode.id)}
	{#if node && !isPropertyGetter(node)}
		<ANodeConfigEditor parameters={bodyConfigParameters(node, nodesById)} />
	{/if}
{/snippet}

{#snippet nodeHeaderContent(graphNode: GraphNode)}
	{@const node = authoredNode(graphNode.id)}
	{#if node && !isPropertyGetter(node)}
		{@const valueTypeParameter = valueTypeConfigParameter(node, nodesById)}
		{#if valueTypeParameter}
			<ANodeValueTypeHeaderControl parameter={valueTypeParameter} />
		{/if}
	{/if}
{/snippet}

{#snippet inputSocketContent(_graphNode: GraphNode, socket: GraphSocket)}
	{@const parameter = inputDefaultParameter(socket)}
	{#if parameter}
		<ANodeSocketDefaultEditor {parameter} />
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
		{onNodeRename}
		{onNodeCollapsedChange}
		{onConnect}
		{canConnect}
		{onBackgroundContextMenu}
		{onCreateRequest}
		{nodeContent}
		{nodeHeaderContent}
		{inputSocketContent}
		{toolbarEnd}
		routeEdgesAroundNodes={true}
		socketLabels="always"
		autoHomeOnMount={false}
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
