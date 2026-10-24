<script lang="ts">
	import GraphCanvas from '../../../components/GraphCanvas.svelte';
	import { GraphPresentationAdapter } from '../../../presentation-document';
	import type { ComponentProps } from 'svelte';
	import { tick } from 'svelte';
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

	const adapter = new GraphPresentationAdapter();
	let graph = $state.raw<GraphPresentationDocument>(adapter.update([], []));

	export function setNodes(nodes: GraphNode[]): void {
		setScene(nodes, []);
	}

	export function setScene(
		nodes: GraphNode[],
		edges: GraphEdge[],
		nextOptions: SceneOptions = {}
	): void {
		options = nextOptions;
		graph = adapter.update(nodes, edges);
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

	export function setScaleScene(count: number): void {
		const nodes: GraphNode[] = Array.from({ length: count }, (_, index) => ({
			id: `n${index}`,
			label: `Node ${index}`,
			position: { x: (index % 256) * 24, y: Math.floor(index / 256) * 24 },
			inputs: [{ id: 'in', label: 'Input' }],
			outputs: [{ id: 'out', label: 'Output' }]
		}));
		const edges: GraphEdge[] = nodes.slice(1).map((node, index) => ({
			id: `e${index}`,
			from: { nodeId: `n${index}`, socketId: 'out' },
			to: { nodeId: node.id, socketId: 'in' }
		}));
		setScene(nodes, edges, { routeEdgesAroundNodes: true });
	}

	const work = () => {
		if (!canvas) throw new Error('Canvas is not mounted.');
		return canvas.presentationWork();
	};
	const difference = (before: ReturnType<typeof work>, after: ReturnType<typeof work>) =>
		Object.fromEntries(
			Object.entries(after).map(([key, value]) => [key, value - before[key as keyof typeof before]])
		);
	const renderedNode = () => {
		const node = document.querySelector<HTMLElement>('article.node[data-node-id="n0"]');
		if (!node) throw new Error('Expected mounted scale node.');
		return node;
	};

	export async function samplePreviews(warmups: number, samples: number) {
		const measurements: {
			flushMs: number;
			frameIntervalMs: number | null;
			work: ReturnType<typeof difference>;
		}[] = [];
		let previousFrame: number | null = null;
		for (let index = 0; index < warmups + samples; index += 1) {
			const frameTime = await new Promise<number>(requestAnimationFrame);
			const before = work();
			const started = performance.now();
			const x = (index % 2) + 1;
			canvas!.previewNodeMoves([{ nodeId: 'n0', position: { x, y: 0 } }]);
			await tick();
			const node = renderedNode();
			if (node.style.left !== `${x}rem` || node.getBoundingClientRect().width <= 0)
				throw new Error('Preview geometry did not render.');
			const flushMs = performance.now() - started;
			if (index >= warmups)
				measurements.push({
					flushMs,
					frameIntervalMs: previousFrame === null ? null : frameTime - previousFrame,
					work: difference(before, work())
				});
			previousFrame = frameTime;
		}
		return measurements;
	}

	export async function probeActivityAndPreview() {
		const before = work();
		const previousRevision = graph.revision;
		const nodes = [...graph.nodes];
		nodes[0] = { ...nodes[0], active: true };
		const started = performance.now();
		graph = adapter.update(nodes, graph.edges);
		await tick();
		if (!renderedNode().classList.contains('active')) throw new Error('Activity did not render.');
		const activity = {
			flushMs: performance.now() - started,
			work: difference(before, work()),
			before: previousRevision,
			after: graph.revision
		};
		const beforePreview = work();
		graph = adapter.update(graph.nodes, graph.edges, 1);
		await tick();
		const preview = { work: difference(beforePreview, work()), revision: graph.revision };
		const beforeResize = work();
		canvas!.previewNodeResize('n0', { width: 20, height: 10 });
		await tick();
		if (renderedNode().style.width !== '20rem') throw new Error('Resize preview did not render.');
		const resize = difference(beforeResize, work());
		canvas!.clearNodePreviews();
		await tick();
		if (renderedNode().style.left !== '0rem')
			throw new Error('Preview cancellation did not restore source geometry.');
		return { activity, preview, resize };
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
