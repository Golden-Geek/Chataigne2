<script lang="ts">
	import GraphCanvas from '../../../components/GraphCanvas.svelte';
	import type { GraphNode, GraphPresentationDocument } from '../../../types';

	let graph = $state.raw<GraphPresentationDocument>({
		revision: { sequence: 0, topology: 0, payload: 0, presentation: 0 },
		nodes: [],
		edges: []
	});

	export function setNodes(nodes: GraphNode[]): void {
		const next = graph.revision.sequence + 1;
		graph = {
			revision: { sequence: next, topology: next, payload: next, presentation: next },
			nodes,
			edges: []
		};
	}
</script>

<div class="fixture">
	<GraphCanvas graphDocument={graph} autoHomeOnMount={false} />
</div>

<style>
	.fixture {
		inline-size: 10rem;
		block-size: 10rem;
	}
</style>
