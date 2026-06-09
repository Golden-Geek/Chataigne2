<script lang="ts">
	import type { GraphConnectionRequest, GraphNodeMove, GraphNodeResize } from 'golden_alchemist_ui';
	import type { PanelProps, PanelState, UiNodeDto } from 'golden_ui';
	import { createUiEditSession } from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		parseAuthoredGraph,
		serializeAuthoredGraph,
		type AuthoredGraphDocument
	} from '../alchemistGraph';
	import AlchemistGraphEditor from './AlchemistGraphEditor.svelte';

	const ACTION_NODE_TYPE = 'state_processor_action';
	const MAPPING_NODE_TYPE = 'state_processor_mapping';
	const AUTHORED_GRAPH_DECL_ID = 'authored_graph';

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	let session = $derived(appState.session);
	let document = $state<AuthoredGraphDocument | null>(null);
	let graphError = $state<string | null>(null);
	let saveStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
	let selectedNodeIds = $state<string[]>([]);
	let persistenceTail = Promise.resolve();

	let processorNodeId = $derived.by(() => {
		const value = panelState.params.processorNodeId;
		if (typeof value === 'number' && Number.isInteger(value)) {
			return value;
		}
		if (typeof value === 'string') {
			const parsed = Number(value);
			return Number.isInteger(parsed) ? parsed : null;
		}
		return null;
	});

	let processor = $derived.by((): UiNodeDto | null => {
		if (!session || processorNodeId === null) {
			return null;
		}
		const node = session.graph.state.nodesById.get(processorNodeId) ?? null;
		return node?.node_type === ACTION_NODE_TYPE || node?.node_type === MAPPING_NODE_TYPE
			? node
			: null;
	});

	let graphParameter = $derived.by((): UiNodeDto | null => {
		if (!session || !processor) {
			return null;
		}
		for (const childId of processor.children) {
			const child = session.graph.state.nodesById.get(childId);
			if (
				child?.decl_id === AUTHORED_GRAPH_DECL_ID &&
				child.data.kind === 'parameter' &&
				child.data.param.value.kind === 'str'
			) {
				return child;
			}
		}
		return null;
	});

	let remoteGraphSource = $derived(
		graphParameter?.data.kind === 'parameter' && graphParameter.data.param.value.kind === 'str'
			? graphParameter.data.param.value.value
			: null
	);

	$effect(() => {
		const source = remoteGraphSource;
		if (source === null) {
			document = null;
			graphError = 'This processor does not expose an authored Alchemist graph.';
			return;
		}
		const parsed = parseAuthoredGraph(source);
		document = parsed;
		graphError = parsed ? null : 'The authored graph is invalid or uses an unsupported version.';
	});

	$effect(() => {
		const title = processor ? `Alchemist: ${processor.meta.label}` : 'Alchemist Editor';
		props.panelApi.setTitle(title);
	});

	const persistDocumentNow = async (
		nextDocument: AuthoredGraphDocument,
		historyLabel: string
	): Promise<void> => {
		if (!session || !graphParameter || graphParameter.data.kind !== 'parameter') {
			throw new Error('Alchemist graph parameter is unavailable');
		}
		const editSession = createUiEditSession(historyLabel, 'alchemist-graph');
		await editSession.begin();
		if (!editSession.active) {
			throw new Error('another edit session is already active');
		}
		try {
			await session.sendIntent({
				kind: 'setParam',
				node: graphParameter.node_id,
				value: { kind: 'str', value: serializeAuthoredGraph(nextDocument) },
				behaviour: graphParameter.data.param.event_behaviour
			});
		} finally {
			await editSession.end();
		}
	};

	const persistDocument = (
		nextDocument: AuthoredGraphDocument,
		historyLabel: string
	): Promise<void> => {
		document = nextDocument;
		graphError = null;
		saveStatus = 'saving';
		const operation = persistenceTail
			.catch(() => undefined)
			.then(() => persistDocumentNow(nextDocument, historyLabel))
			.then(() => {
				saveStatus = 'saved';
			})
			.catch((error: unknown) => {
				saveStatus = 'error';
				console.error('failed to persist Alchemist graph', error);
				throw error;
			});
		persistenceTail = operation.catch(() => undefined);
		return operation;
	};

	const moveNodes = (moves: GraphNodeMove[]): Promise<void> => {
		if (!document || moves.length === 0) {
			return Promise.resolve();
		}
		const positions = new Map(moves.map((move) => [move.nodeId, move.position]));
		const nextDocument: AuthoredGraphDocument = {
			...document,
			nodes: document.nodes.map((node) => {
				const position = positions.get(node.id);
				return position ? { ...node, x: position.x, y: position.y } : node;
			})
		};
		return persistDocument(
			nextDocument,
			moves.length === 1 ? 'Move Alchemist node' : `Move ${moves.length} Alchemist nodes`
		);
	};

	const resizeNode = (resize: GraphNodeResize): Promise<void> => {
		if (!document) {
			return Promise.resolve();
		}
		const nextDocument: AuthoredGraphDocument = {
			...document,
			nodes: document.nodes.map((node) =>
				node.id === resize.nodeId
					? { ...node, width: resize.size.width, height: resize.size.height }
					: node
			)
		};
		return persistDocument(nextDocument, 'Resize Alchemist node');
	};

	const connectNodes = (connection: GraphConnectionRequest): void => {
		if (!document) {
			return;
		}
		const duplicate = document.edges.some(
			(edge) =>
				edge.from.nodeId === connection.from.nodeId &&
				edge.from.socketId === connection.from.socketId &&
				edge.to.nodeId === connection.to.nodeId &&
				edge.to.socketId === connection.to.socketId
		);
		if (duplicate) {
			return;
		}
		const edgeId = `${connection.from.nodeId}:${connection.from.socketId}->${connection.to.nodeId}:${connection.to.socketId}`;
		const nextDocument: AuthoredGraphDocument = {
			...document,
			edges: [
				...document.edges.filter(
					(edge) =>
						edge.to.nodeId !== connection.to.nodeId || edge.to.socketId !== connection.to.socketId
				),
				{
					id: edgeId,
					from: { ...connection.from },
					to: { ...connection.to }
				}
			]
		};
		void persistDocument(nextDocument, 'Connect Alchemist nodes');
	};

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};
</script>

<section class="alchemist-editor-panel" aria-label={panelState.title}>
	<header>
		<div>
			<strong>{processor?.meta.label ?? 'No processor selected'}</strong>
			<span>
				{processor?.node_type === ACTION_NODE_TYPE
					? 'Action formula'
					: processor?.node_type === MAPPING_NODE_TYPE
						? 'Mapping formula'
						: 'Select an Action or Mapping processor'}
			</span>
		</div>
		<span class:error={saveStatus === 'error'} class="save-status">
			{saveStatus === 'saving'
				? 'Saving...'
				: saveStatus === 'saved'
					? 'Saved'
					: saveStatus === 'error'
						? 'Save failed'
						: ''}
		</span>
	</header>

	<div class="editor-content">
		{#if document}
			<AlchemistGraphEditor
				{document}
				{selectedNodeIds}
				onSelectionChange={(nodeIds) => {
					selectedNodeIds = nodeIds;
				}}
				onNodesMove={moveNodes}
				onNodeResize={resizeNode}
				onConnect={connectNodes} />
		{:else}
			<div class="empty-state">
				<strong>Alchemist graph unavailable</strong>
				<p>{graphError ?? 'Select an Action or Mapping processor from the inspector.'}</p>
			</div>
		{/if}
	</div>
</section>

<style>
	.alchemist-editor-panel {
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.8rem;
		min-block-size: 2.5rem;
		padding: 0.45rem 0.75rem;
		border-block-end: 0.06rem solid var(--gc-color-border);
		background: var(--gc-color-background-soft);
	}

	header div {
		display: grid;
		gap: 0.12rem;
	}

	header strong {
		font-size: 0.78rem;
	}

	header span {
		color: color-mix(in srgb, var(--gc-color-text) 62%, transparent);
		font-size: 0.64rem;
	}

	.save-status {
		min-inline-size: 4rem;
		text-align: end;
	}

	.save-status.error {
		color: var(--gc-color-error);
	}

	.editor-content {
		min-inline-size: 0;
		min-block-size: 0;
	}

	.empty-state {
		display: grid;
		place-content: center;
		block-size: 100%;
		padding: 2rem;
		text-align: center;
	}

	.empty-state p {
		max-inline-size: 30rem;
		margin: 0.4rem 0 0;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.72rem;
	}
</style>
