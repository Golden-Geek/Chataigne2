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

	const PROCESSOR_NODE_TYPE = 'state_processor';
	const AUTHORED_GRAPH_DECL_ID = 'authored_graph';
	const BUILTIN_FORMULA_TYPES = new Set(['alchemist_formula_action', 'alchemist_formula_mapping']);

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
	let graphState = $derived(session?.graph.state ?? null);
	let document = $state<AuthoredGraphDocument | null>(null);
	let graphError = $state<string | null>(null);
	let saveStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
	let selectedNodeIds = $state<string[]>([]);
	let persistenceTail = Promise.resolve();

	const isFormulaProcessor = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.node_type === PROCESSOR_NODE_TYPE;

	let requestedProcessorNodeId = $derived.by(() => {
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

	let processorNodes = $derived.by((): UiNodeDto[] => {
		if (!session) {
			return [];
		}
		return [...session.graph.state.nodesById.values()]
			.filter(isFormulaProcessor)
			.sort((left, right) => left.node_id - right.node_id);
	});

	let selectedProcessor = $derived.by((): UiNodeDto | null => {
		if (!session) {
			return null;
		}
		for (const selectedNodeId of session.selectedNodesIds) {
			let currentNodeId: number | undefined = selectedNodeId;
			while (currentNodeId !== undefined) {
				const current = session.graph.state.nodesById.get(currentNodeId);
				if (isFormulaProcessor(current)) {
					return current;
				}
				currentNodeId = session.graph.state.parentById.get(currentNodeId);
			}
		}
		return null;
	});

	let processor = $derived.by((): UiNodeDto | null => {
		if (!session) {
			return null;
		}
		const requested =
			requestedProcessorNodeId === null
				? null
				: session.graph.state.nodesById.get(requestedProcessorNodeId);
		return isFormulaProcessor(requested)
			? requested
			: (selectedProcessor ?? processorNodes[0] ?? null);
	});

	const findFormulaNode = (processorNode: UiNodeDto): UiNodeDto | null => {
		if (!graphState) return null;
		for (const childId of processorNode.children) {
			const child = graphState.nodesById.get(childId);
			if (
				child?.decl_id === 'formula_uuid' &&
				child.data.kind === 'parameter' &&
				child.data.param.value.kind === 'str'
			) {
				const uuid = child.data.param.value.value;
				for (const n of graphState.nodesById.values()) {
					if (n.uuid === uuid) return n;
				}
			}
		}
		return null;
	};

	let formulaNode = $derived(processor ? findFormulaNode(processor) : null);
	let formulaIsBuiltin = $derived(formulaNode ? BUILTIN_FORMULA_TYPES.has(formulaNode.node_type) : false);
	let formulaKind = $derived(
		formulaNode
			? formulaIsBuiltin
				? 'Built-in'
				: (formulaNode.meta.label ?? 'Custom')
			: null
	);

	let processorSlots = $derived.by((): UiNodeDto[] => {
		if (!processor || !graphState) return [];
		return processor.children
			.map((id) => graphState.nodesById.get(id))
			.filter((n): n is UiNodeDto => n != null && n.data.kind === 'node');
	});

	let graphParameter = $derived.by((): UiNodeDto | null => {
		if (!session || !formulaNode || formulaIsBuiltin) return null;
		for (const childId of formulaNode.children) {
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
		if (!processor) {
			document = null;
			graphError =
				processorNodes.length === 0
					? 'Create a processor to start editing.'
					: 'Select a processor.';
			return;
		}
		if (formulaIsBuiltin) {
			document = null;
			graphError = null;
			return;
		}
		if (source === null) {
			document = null;
			graphError = 'No authored graph found for this custom formula.';
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

	const selectProcessor = (event: Event): void => {
		const select = event.currentTarget as HTMLSelectElement;
		const processorNodeId = Number(select.value);
		if (!Number.isInteger(processorNodeId)) {
			return;
		}
		const params = {
			...panelState.params,
			processorNodeId
		};
		updatedPanelState = {
			...panelState,
			params
		};
		props.panelApi.updateParams(params);
		selectedNodeIds = [];
		saveStatus = 'idle';
	};

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
				{formulaKind ? `${formulaKind} formula` : processor ? 'Unknown formula' : 'Select a processor'}
			</span>
		</div>
		{#if processorNodes.length > 0}
			<label class="processor-picker">
				<span>Processor</span>
				<select value={processor?.node_id ?? ''} onchange={selectProcessor}>
					{#each processorNodes as option (option.node_id)}
						<option value={option.node_id}>
							{option.meta.label} ({(findFormulaNode(option)?.meta.label) ?? '?'})
						</option>
					{/each}
				</select>
			</label>
		{/if}
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
		{:else if processor && formulaIsBuiltin}
			<div class="builtin-formula-view">
				<div class="builtin-header">
					<strong>{formulaNode?.meta.label ?? 'Built-in'} formula</strong>
					<p>This formula is defined in code and cannot be edited as a graph.</p>
				</div>
				{#if processorSlots.length > 0}
					<div class="slot-grid">
						{#each processorSlots as slot (slot.node_id)}
							<div class="slot-block">{slot.meta.label}</div>
						{/each}
					</div>
				{/if}
			</div>
		{:else}
			<div class="empty-state">
				<strong>Alchemist graph unavailable</strong>
				<p>{graphError ?? 'Select a processor from the inspector.'}</p>
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

	.processor-picker {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		margin-inline-start: auto;
	}

	.processor-picker select {
		min-inline-size: 10rem;
		max-inline-size: 20rem;
		min-block-size: 1.65rem;
		padding: 0.2rem 1.6rem 0.2rem 0.45rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.68rem;
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

	.builtin-formula-view {
		display: flex;
		flex-direction: column;
		align-items: center;
		block-size: 100%;
		padding: 2rem;
		gap: 1.5rem;
	}

	.builtin-header {
		text-align: center;
	}

	.builtin-header strong {
		font-size: 0.85rem;
	}

	.builtin-header p {
		margin: 0.3rem 0 0;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.7rem;
	}

	.slot-grid {
		display: flex;
		flex-wrap: wrap;
		gap: 0.75rem;
		justify-content: center;
	}

	.slot-block {
		min-inline-size: 7rem;
		padding: 0.75rem 1rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.45rem;
		background: var(--gc-color-background-soft);
		font-size: 0.75rem;
		font-weight: 500;
		text-align: center;
	}
</style>
