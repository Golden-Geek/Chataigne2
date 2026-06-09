<script lang="ts">
	import {
		GraphCanvas,
		type GraphConnectionRequest,
		type GraphNodeMove,
		type GraphNodeResize
	} from 'golden_alchemist_ui';
	import { toGraphEdges, toGraphNodes, type AuthoredGraphDocument } from '../alchemistGraph';

	let {
		document,
		selectedNodeIds = [],
		onSelectionChange,
		onNodesMove,
		onNodeResize,
		onConnect
	}: {
		document: AuthoredGraphDocument;
		selectedNodeIds?: string[];
		onSelectionChange?: (nodeIds: string[]) => void;
		onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
		onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
		onConnect?: (connection: GraphConnectionRequest) => void;
	} = $props();

	let nodes = $derived(toGraphNodes(document));
	let edges = $derived(toGraphEdges(document));
</script>

<div class="alchemist-graph-editor">
	<GraphCanvas
		{nodes}
		{edges}
		{selectedNodeIds}
		{onSelectionChange}
		{onNodesMove}
		{onNodeResize}
		{onConnect}
		routeEdgesAroundNodes={true}
		socketLabels="always"
		emptyLabel="This formula has no authored nodes." />
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
