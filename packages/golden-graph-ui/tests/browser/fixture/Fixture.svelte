<script lang="ts">
	import GraphCanvas from '../../../components/GraphCanvas.svelte';
	import { GraphPresentationAdapter } from '../../../presentation-document';
	import type { ComponentProps } from 'svelte';
	import { tick } from 'svelte';
	import type {
		GraphEdge,
		GraphNode,
		GraphPresentationDocument,
		GraphNodeMove,
		GraphNodeResize
	} from '../../../types';
	import { createQualification } from './qualification';
	import { qualifyDense } from './qualification-dense';
	import { createNavigationQualification } from './navigation-qualification';
	import { createKeyboardQualification } from './keyboard-qualification';

	type SceneOptions = Pick<
		ComponentProps<typeof GraphCanvas>,
		| 'selectedNodeIds'
		| 'selectedEdgeIds'
		| 'socketLabels'
		| 'routeEdgesAroundNodes'
		| 'initialCamera'
	>;
	let canvas = $state<ReturnType<typeof GraphCanvas>>();
	let canvasMounted = $state(true);
	export async function replaceCanvasWhileSearching() {
		const previous = canvas!;
		await previous.showNodeFinder();
		const pending = previous.searchNodes('retired unmatched search');
		canvasMounted = false;
		await tick();
		await pending;
		const staleReveal = await previous.revealNode('n0');
		canvasMounted = true;
		await tick();
		return { staleReveal, finderOpen: canvas!.nodeFinderState().open };
	}
	let options = $state.raw<SceneOptions>({});
	let canvasSize = $state.raw({ width: 10, height: 10 });
	let interactionScope = $state.raw<object>({});
	let keyboardReadOnly = $state(false);
	let keyboardPolicy = $state(true);
	let moveMode: 'normal' | 'reject' | 'hold' = 'normal';
	let heldMoves: ((reason?: unknown) => void)[] = [];
	let connections: {
		from: { nodeId: string; socketId: string };
		to: { nodeId: string; socketId: string };
	}[] = [];
	let connectionMode: 'normal' | 'reject' | 'hold' = 'normal';
	let acknowledgedConnections = false;
	let collapseMode: 'normal' | 'reject' | 'hold' = 'normal';
	let heldCollapses: ((reason?: unknown) => void)[] = [];
	let heldConnections: ((reason?: unknown) => void)[] = [];
	const captureConnection = (connection: (typeof connections)[number]): void | Promise<void> => {
		if (connectionMode === 'reject')
			return Promise.reject(new Error('Controlled connection rejection'));
		if (connectionMode === 'hold')
			return new Promise<void>((_resolve, reject) => {
				heldConnections.push(reject);
			});
		connections = [...connections, connection];
		if (acknowledgedConnections)
			graph = adapter.update(graph.nodes, [
				...graph.edges.filter(
					(edge) =>
						edge.to.nodeId !== connection.to.nodeId || edge.to.socketId !== connection.to.socketId
				),
				{ ...connection, id: 'acknowledged-keyboard-connection' }
			]);
	};

	export function setCanvasSize(width: number, height: number): void {
		canvasSize = { width, height };
	}

	export function routingWork() {
		if (!canvas) throw new Error('Canvas is not mounted.');
		return canvas.routingWork();
	}

	export async function settleRouting() {
		for (let frame = 0; frame < 1024; frame += 1) {
			await tick();
			if (
				routingWork().pendingRoutes === 0 &&
				canvas!.mountingWork().nodes === 0 &&
				canvas!.mountingWork().edges === 0
			)
				return routingWork();
			await new Promise<number>(requestAnimationFrame);
		}
		throw new Error('Routing queue did not settle.');
	}

	const adapter = new GraphPresentationAdapter();
	let graph = $state.raw<GraphPresentationDocument>(adapter.update([], []));
	const acknowledgeCollapse = async (nodeId: string, collapsed: boolean) => {
		if (collapseMode === 'reject') throw new Error('Controlled collapse rejection');
		if (collapseMode === 'hold')
			return new Promise<void>((_resolve, reject) => heldCollapses.push(reject));
		graph = adapter.update(
			graph.nodes.map((node) => (node.id === nodeId ? { ...node, collapsed } : node)),
			graph.edges
		);
	};
	const acknowledgeMoves = async (moves: GraphNodeMove[]) => {
		if (moveMode === 'reject') throw new Error('Controlled move rejection');
		if (moveMode === 'hold')
			return new Promise<void>((_resolve, reject) => {
				heldMoves.push(reject);
			});
		const positions = new Map(moves.map((move) => [move.nodeId, move.position]));
		graph = adapter.update(
			graph.nodes.map((node) =>
				positions.has(node.id) ? { ...node, position: positions.get(node.id)! } : node
			),
			graph.edges
		);
	};
	const acknowledgeResize = async (resize: GraphNodeResize) => {
		graph = adapter.update(
			graph.nodes.map((node) =>
				node.id === resize.nodeId
					? { ...node, size: resize.mode === 'custom' ? resize.size : undefined }
					: node
			),
			graph.edges
		);
	};
	const qualificationContext = {
		canvas: () => {
			if (!canvas) throw new Error('Canvas is not mounted.');
			return canvas;
		},
		graph: () => graph,
		publish: (nodes: readonly GraphNode[], edges: readonly GraphEdge[]) => {
			graph = adapter.update(nodes, edges);
		},
		select: (selectedNodeIds: string[]) => {
			options = { routeEdgesAroundNodes: true, selectedNodeIds };
		},
		size: setCanvasSize
	};
	const qualification = createQualification(qualificationContext);
	const keyboardQualification = createKeyboardQualification({
		...qualificationContext,
		configure() {
			interactionScope = {};
			acknowledgedConnections = true;
			moveMode = collapseMode = connectionMode = 'normal';
			keyboardReadOnly = false;
			keyboardPolicy = true;
			connections = [];
		}
	});
	export const setupKeyboardQualification = keyboardQualification.setup;
	export const runKeyboardQualification = keyboardQualification.run;
	const discovery = createNavigationQualification(qualificationContext);
	export const setupDiscovery = discovery.setup;
	export const runDiscoveryAction = discovery.run;
	export const discoveryCorrectness = discovery.correctness;
	export const showNodeFinder = () => canvas!.showNodeFinder();
	export const searchNodes = (query: string) => canvas!.searchNodes(query);
	export const nodeFinderState = () => canvas!.nodeFinderState();
	export const setupQualification = qualification.setup;
	export const runQualificationAction = qualification.run;
	export const runDenseQualification = (varied: boolean, diagnostic = false) =>
		qualifyDense(qualificationContext, varied, diagnostic ? { warmups: 0, samples: 1 } : undefined);

	export function setNodes(nodes: GraphNode[]): void {
		setScene(nodes, []);
	}
	const keyboardFrames = async () => {
		await tick();
		await new Promise(requestAnimationFrame);
		await tick();
		await new Promise(requestAnimationFrame);
	};
	export async function setupKeyboard() {
		acknowledgedConnections = false;
		collapseMode = 'normal';
		heldCollapses = [];
		connectionMode = 'normal';
		heldConnections = [];
		moveMode = 'normal';
		heldMoves = [];
		connections = [];
		keyboardReadOnly = false;
		keyboardPolicy = true;
		interactionScope = {};
		setCanvasSize(70, 36);
		setScene(
			[
				{
					id: 'a',
					label: 'Source',
					inputs: [],
					outputs: [{ id: 'out', label: 'Output' }],
					position: { x: 1, y: 2 }
				},
				{
					id: 'b',
					label: 'Destination',
					inputs: [{ id: 'in', label: 'Input' }],
					outputs: [],
					position: { x: 24, y: 2 }
				},
				{
					id: 'c',
					label: 'Blocked',
					inputs: [{ id: 'no', label: 'Disabled', compatible: false }],
					outputs: [],
					position: { x: 42, y: 2 }
				}
			],
			[],
			{ selectedNodeIds: ['a', 'b'], initialCamera: { x: 0, y: 0, zoom: 1 } }
		);
		await keyboardFrames();
	}
	export async function keyboardMove(key: string, large = false) {
		const accepted = canvas!.moveSelectionWithKeyboard(key, large);
		await keyboardFrames();
		return accepted;
	}
	export async function keyboardSocket(
		node: string,
		socket: string,
		direction: 'input' | 'output'
	) {
		const accepted = canvas!.activateSocketWithKeyboard(node, socket, direction);
		await keyboardFrames();
		return accepted;
	}
	export async function keyboardSelect(node: string, additive = false) {
		const accepted = canvas!.selectNodeWithKeyboard(node, additive);
		await keyboardFrames();
		return accepted;
	}
	export async function keyboardCancel() {
		const accepted = canvas!.cancelKeyboardConnection();
		await keyboardFrames();
		return accepted;
	}
	export async function keyboardCollapse(nodeId: string) {
		const accepted = canvas!.toggleNodeCollapsed(nodeId);
		await keyboardFrames();
		return accepted;
	}
	export async function keyboardExpand(
		nodeId: string,
		socketId: string,
		direction: 'input' | 'output'
	) {
		const accepted = canvas!.toggleSocketExpanded(nodeId, socketId, direction);
		await keyboardFrames();
		return accepted;
	}
	export const setCollapseMode = (mode: typeof collapseMode) => {
		collapseMode = mode;
	};
	export async function rejectHeldCollapse(index: number) {
		heldCollapses[index]?.(new Error('Controlled stale collapse rejection'));
		await keyboardFrames();
	}
	export async function setupSockets(mode: 'body' | 'header' | 'collapsed') {
		await setupKeyboard();
		const vector = (direction: 'input' | 'output') => ({
			id: 'vector',
			label: `${direction} vector`,
			children: [
				{
					id: 'x',
					label: `${direction} X`,
					parentId: 'vector',
					children: [{ id: 'leaf', label: `${direction} leaf`, parentId: 'x' }]
				},
				{ id: 'y', label: `${direction} Y`, parentId: 'vector' }
			]
		});
		graph = adapter.update(
			graph.nodes.map((node) => ({
				...node,
				collapsed: mode === 'collapsed',
				socketPlacement: mode === 'header' ? 'header' : 'body',
				inputs: node.id === 'b' ? [{ id: 'in', label: 'Input' }, vector('input')] : node.inputs,
				headerInputs:
					node.id === 'b' && mode === 'body' ? [{ id: 'header', label: 'Header' }] : undefined,
				outputs:
					node.id === 'a'
						? [
								{ id: 'out', label: 'Output' },
								vector('output'),
								{ id: 'disabled', label: 'Disabled output', compatible: false }
							]
						: node.id === 'b'
							? [vector('output')]
							: node.outputs
			})),
			[]
		);
		await keyboardFrames();
	}
	export async function setSocketConnection(socketId: string | null) {
		graph = adapter.update(
			graph.nodes,
			socketId
				? [
						{
							id: 'controlled-input',
							from: { nodeId: 'a', socketId: 'out' },
							to: { nodeId: 'b', socketId }
						}
					]
				: []
		);
		await keyboardFrames();
	}
	export const keyboardInspect = () => ({
		state: canvas!.keyboardState(),
		selected: options.selectedNodeIds,
		positions: graph.nodes.map((node) => ({ id: node.id, ...node.position })),
		connections
	});
	export async function keyboardMode(mode: 'normal' | 'reject' | 'hold' | 'readonly' | 'blocked') {
		if (mode === 'readonly') keyboardReadOnly = true;
		else if (mode === 'blocked') keyboardPolicy = false;
		else {
			moveMode = mode;
			keyboardReadOnly = false;
			keyboardPolicy = true;
		}
		await keyboardFrames();
	}
	export async function rejectHeldMove(index: number) {
		heldMoves[index]?.(new Error('Controlled stale rejection'));
		await keyboardFrames();
	}
	export async function replaceKeyboardScope() {
		interactionScope = {};
		graph = adapter.update(
			graph.nodes.map((node) => ({
				...node,
				position: { x: node.position.x + 10, y: node.position.y }
			})),
			graph.edges
		);
		await keyboardFrames();
	}
	export async function repeatMovePreview() {
		canvas!.previewNodeMoves([{ nodeId: 'a', position: { x: 5, y: 2 } }]);
		canvas!.commitNodeMovePreview(['a']);
		await keyboardFrames();
	}
	export async function keyboardPublication(topology = false) {
		graph = adapter.update(
			graph.nodes.map((node) =>
				node.id !== 'a'
					? node
					: topology
						? { ...node, outputs: [...node.outputs, { id: 'extra', label: 'Extra' }] }
						: { ...node, active: !node.active }
			),
			graph.edges
		);
		await keyboardFrames();
	}
	export const connectionFailureMode = (mode: typeof connectionMode) => {
		connectionMode = mode;
	};
	export async function rejectHeldConnection(index: number) {
		heldConnections[index]?.(new Error('Controlled connection failure'));
		await keyboardFrames();
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
		const beforeFinish = routingWork();
		canvas!.clearNodePreviews();
		await tick();
		if (renderedNode().style.left !== '0rem')
			throw new Error('Preview cancellation did not restore source geometry.');
		const finishedRouting = await settleRouting();
		return {
			activity,
			preview,
			resize,
			finish: { before: beforeFinish, after: finishedRouting, projection: work() }
		};
	}
</script>

<div
	class="fixture"
	style:--fixture-width={`${canvasSize.width}rem`}
	style:--fixture-height={`${canvasSize.height}rem`}>
	{#if canvasMounted}
		<GraphCanvas
			bind:this={canvas}
			{interactionScope}
			graphDocument={graph}
			autoHomeOnMount={false}
			onNodesMove={keyboardReadOnly ? undefined : acknowledgeMoves}
			onGraphSelectionChange={(selectedNodeIds, selectedEdgeIds) => {
				options = { ...options, selectedNodeIds, selectedEdgeIds };
			}}
			onConnect={keyboardReadOnly ? undefined : captureConnection}
			onNodeCollapsedChange={keyboardReadOnly ? undefined : acknowledgeCollapse}
			canConnect={() => keyboardPolicy}
			onNodeResize={acknowledgeResize}
			{...options}>
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
	{/if}
</div>

<style>
	.fixture {
		inline-size: var(--fixture-width);
		block-size: var(--fixture-height);
	}
</style>
