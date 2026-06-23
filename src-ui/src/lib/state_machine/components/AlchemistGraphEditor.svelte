<script lang="ts">
	import { type Snippet } from 'svelte';
	import {
		GraphCanvas,
		type GraphCamera,
		type GraphConnectionRequest,
		type GraphNode,
		type GraphNodeCreationRequest,
		type GraphNodeMove,
		type GraphNodePosition,
		type GraphNodeResize,
		type GraphSocket,
		type GraphViewportInset
	} from 'golden_alchemist_ui';
	import type { NodeId, UiCreatableUserItem, UiNodeDto } from 'golden_ui';
	import { canConnectGraphConnection, toGraphEdges, toGraphNodes } from '../alchemistGraph';
	import type { FormulaOutputPreviewChip } from '../preview/formulaOutputPreviewStore.svelte';
	import ANodeSocketDefaultEditor from './ANodeSocketDefaultEditor.svelte';
	import ANodeOutputValueChip from './ANodeOutputValueChip.svelte';

	let {
		formula,
		nodesById,
		selectedNodeIds = [],
		selectedEdgeIds = [],
		activeSocketRefs = new Set<string>(),
		outputPreviews = new Map<string, FormulaOutputPreviewChip>(),
		catalogItems = [],
		onGraphSelectionChange,
		onNodesMove,
		onNodeResize,
		onNodeRename,
		onNodeCollapsedChange,
		onNodeEnabledChange,
		onConnect,
		onBackgroundContextMenu,
		onCreateRequest,
		initialCamera,
		onCameraChange,
		viewportInset,
		autoWire = true,
		toolbarEnd
	}: {
		formula: UiNodeDto;
		nodesById: ReadonlyMap<NodeId, UiNodeDto>;
		selectedNodeIds?: string[];
		selectedEdgeIds?: string[];
		activeSocketRefs?: ReadonlySet<string>;
		outputPreviews?: ReadonlyMap<string, FormulaOutputPreviewChip>;
		catalogItems?: UiCreatableUserItem[];
		onGraphSelectionChange?: (nodeIds: string[], edgeIds: string[]) => void;
		onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
		onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
		onNodeRename?: (nodeId: string, label: string) => void | Promise<void>;
		onNodeCollapsedChange?: (nodeId: string, collapsed: boolean) => void | Promise<void>;
		onNodeEnabledChange?: (nodeId: string, enabled: boolean) => void | Promise<void>;
		onConnect?: (connection: GraphConnectionRequest) => void;
		onBackgroundContextMenu?: (event: MouseEvent, position: GraphNodePosition) => void;
		onCreateRequest?: (request: GraphNodeCreationRequest) => void;
		initialCamera?: GraphCamera;
		onCameraChange?: (camera: GraphCamera) => void;
		viewportInset?: GraphViewportInset;
		autoWire?: boolean;
		toolbarEnd?: Snippet;
	} = $props();

	let activeNodeIds = $derived.by(() => {
		const result = new Set<string>();
		for (const ref of activeSocketRefs) {
			const separator = ref.indexOf(':');
			if (separator <= 0) continue;
			result.add(ref.slice(0, separator));
		}
		return result;
	});
	let nodes = $derived(
		toGraphNodes(formula, nodesById, catalogItems).map((node) =>
			activeNodeIds.has(node.id) ? { ...node, active: true } : node
		)
	);
	let edges = $derived(toGraphEdges(formula, nodesById, activeSocketRefs));
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
		pendingHome = !initialCamera && !framedFormulaIds.has(formulaId);
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

	const inputDefaultParameter = (socket: GraphSocket): UiNodeDto | null =>
		socket.defaultParamId ? (nodesById.get(Number(socket.defaultParamId)) ?? null) : null;
	const outputPreview = (
		graphNode: GraphNode,
		socket: GraphSocket
	): FormulaOutputPreviewChip | null => {
		const key = `${graphNode.id}:${socket.id}`;
		const preview = outputPreviews.get(key);
		return preview ? { ...preview, active: activeSocketRefs.has(key) } : null;
	};
	const inputPreview = (graphNode: GraphNode, socket: GraphSocket): FormulaOutputPreviewChip | null =>
		graphNode.outputs.some((output) => output.id === socket.id) ? null : outputPreview(graphNode, socket);
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

{#snippet inputSocketContent(graphNode: GraphNode, socket: GraphSocket)}
	{@const parameter = inputDefaultParameter(socket)}
	{#if parameter}
		<ANodeSocketDefaultEditor {parameter} />
	{/if}
	<ANodeOutputValueChip preview={inputPreview(graphNode, socket)} />
{/snippet}

{#snippet outputSocketContent(graphNode: GraphNode, socket: GraphSocket)}
	<ANodeOutputValueChip preview={outputPreview(graphNode, socket)} />
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
		{onNodeEnabledChange}
		{onConnect}
		{canConnect}
		{onBackgroundContextMenu}
		{onCreateRequest}
		{initialCamera}
		{onCameraChange}
		{viewportInset}
		{inputSocketContent}
		{outputSocketContent}
		{toolbarEnd}
		routeEdgesAroundNodes={autoWire}
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
