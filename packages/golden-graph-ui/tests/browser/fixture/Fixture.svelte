<script lang="ts">
	import GraphCanvas from '../../../components/GraphCanvas.svelte';
	import type { ComponentProps } from 'svelte';
	import type { GraphEdge, GraphNode, GraphPresentationDocument } from '../../../types';

	type SceneOptions = Pick<
		ComponentProps<typeof GraphCanvas>,
		| 'selectedNodeIds'
		| 'selectedEdgeIds'
		| 'socketLabels'
		| 'routeEdgesAroundNodes'
		| 'initialCamera'
	>;
	let canvas = $state<ReturnType<typeof GraphCanvas>>();
	let options = $state.raw<SceneOptions>({});

	let graph = $state.raw<GraphPresentationDocument>({
		revision: { sequence: 0, topology: 0, payload: 0, presentation: 0 },
		nodes: [],
		edges: []
	});

	export function setNodes(nodes: GraphNode[]): void {
		setScene(nodes, []);
	}

	export function setScene(
		nodes: GraphNode[],
		edges: GraphEdge[],
		nextOptions: SceneOptions = {}
	): void {
		options = nextOptions;
		const next = graph.revision.sequence + 1;
		graph = {
			revision: { sequence: next, topology: next, payload: next, presentation: next },
			nodes,
			edges
		};
	}

	export function frameSelection(): boolean {
		return canvas?.frameSelection() ?? false;
	}

	export function home(): boolean {
		return canvas?.home() ?? false;
	}

	export function coordinates(clientX: number, clientY: number) {
		return {
			world: canvas?.clientToWorld(clientX, clientY),
			center: canvas?.viewportCenter()
		};
	}
</script>

<div class="fixture">
	<GraphCanvas bind:this={canvas} graphDocument={graph} autoHomeOnMount={false} {...options}>
		{#snippet nodeContent(node)}
			<span data-fixture-node>{node.label}</span>
		{/snippet}
		{#snippet nodeHeaderContent(node)}
			<span data-fixture-header>{node.label}</span>
		{/snippet}
		{#snippet inputSocketContent(node, socket)}
			<span data-fixture-input>{node.id}:{socket.id}</span>
		{/snippet}
		{#snippet outputSocketContent(node, socket)}
			<span data-fixture-output>{node.id}:{socket.id}</span>
		{/snippet}
		{#snippet worldContent(context)}
			<span data-fixture-world>{context.remPx}</span>
		{/snippet}
		{#snippet toolbarEnd()}
			<span data-fixture-toolbar>Fixture</span>
		{/snippet}
	</GraphCanvas>
</div>

<style>
	.fixture {
		inline-size: 10rem;
		block-size: 10rem;
	}
</style>
