<script lang="ts">
	import { onDestroy } from 'svelte';
	import ParameterInspector from './ParameterInspector.svelte';
	import type {
		EventTime,
		NodeId,
		UiClient,
		UiEditIntent,
		UiEventBatch,
		UiNodeDto,
		UiSnapshot,
		UiSubscriptionScope
	} from '../types';

	let {
		client,
		selectedNodeId = null,
		fallbackNode = null,
		onIntent
	}: {
		client: UiClient;
		selectedNodeId?: NodeId | null;
		fallbackNode?: UiNodeDto | null;
		onIntent: (intent: UiEditIntent) => void;
	} = $props();

	let scopedNodesById = $state<Map<NodeId, UiNodeDto>>(new Map());
	let scopedChildrenById = $state<Map<NodeId, NodeId[]>>(new Map());
	let lastEventTime = $state<EventTime | undefined>(undefined);
	let isLoading = $state(false);
	let loadError = $state<string | null>(null);

	let activeRunId = 0;
	let activeUnsubscribe: (() => void) | null = null;
	let activeScopeNodeId: NodeId | null = null;
	let activeClientRef: UiClient | null = null;

	const stopActiveSubscription = (): void => {
		if (activeUnsubscribe) {
			activeUnsubscribe();
			activeUnsubscribe = null;
		}
	};

	const inspectorScopeForNode = (root: NodeId): UiSubscriptionScope => ({
		kind: 'subtree',
		root,
		max_depth: 1
	});

	const rebuildFromSnapshot = (snapshot: UiSnapshot): void => {
		const nextNodesById = new Map<NodeId, UiNodeDto>();
		const nextChildrenById = new Map<NodeId, NodeId[]>();
		for (const node of snapshot.nodes) {
			nextNodesById.set(node.node_id, node);
			nextChildrenById.set(node.node_id, [...node.children]);
		}
		scopedNodesById = nextNodesById;
		scopedChildrenById = nextChildrenById;
		lastEventTime = snapshot.at;
	};

	const selectedNode = $derived.by(() => {
		if (selectedNodeId === null) {
			return null;
		}
		const fromScoped = scopedNodesById.get(selectedNodeId) ?? null;
		if (fromScoped) {
			return fromScoped;
		}
		if (fallbackNode && fallbackNode.node_id === selectedNodeId) {
			return fallbackNode;
		}
		return null;
	});

	const selectedNodeChildren = $derived.by(() => {
		if (!selectedNode) {
			return [];
		}
		return scopedChildrenById.get(selectedNode.node_id) ?? selectedNode.children ?? [];
	});

	const selectedNodeChildParameters = $derived.by(() =>
		selectedNodeChildren
			.map((childId) => scopedNodesById.get(childId))
			.filter(
				(node): node is UiNodeDto =>
					node !== undefined && node.data.kind === 'parameter'
			)
	);

	const refreshScopeSnapshot = async (
		scope: UiSubscriptionScope,
		runId: number,
		reason: string
	): Promise<void> => {
		try {
			const snapshot = await client.snapshot(scope);
			if (runId !== activeRunId) {
				return;
			}
			rebuildFromSnapshot(snapshot);
			loadError = null;
			console.info(
				`[ui inspector] refreshed subtree scope after ${reason}: ${JSON.stringify(scope)}`
			);
		} catch (error) {
			if (runId !== activeRunId) {
				return;
			}
			loadError = error instanceof Error ? error.message : 'unknown snapshot refresh error';
		}
	};

	const applyScopedBatch = (
		batch: UiEventBatch,
		runId: number,
		refresh: (reason: string) => Promise<void>
	): void => {
		if (runId !== activeRunId) {
			return;
		}

		let needsSnapshotRefresh = false;
		let touchedNodes = false;
		const nextNodesById = new Map(scopedNodesById);

		for (const event of batch.events) {
			switch (event.kind.kind) {
				case 'paramChanged': {
					const node = nextNodesById.get(event.kind.param);
					if (!node || node.data.kind !== 'parameter') {
						needsSnapshotRefresh = true;
						break;
					}
					const updatedNode: UiNodeDto = {
						...node,
						data: {
							kind: 'parameter',
							param: {
								...node.data.param,
								value: event.kind.new_value
							}
						}
					};
					nextNodesById.set(event.kind.param, updatedNode);
					touchedNodes = true;
					break;
				}
				case 'metaChanged': {
					const node = nextNodesById.get(event.kind.node);
					if (!node) {
						needsSnapshotRefresh = true;
						break;
					}
					nextNodesById.set(event.kind.node, {
						...node,
						meta: {
							...node.meta,
							...event.kind.patch
						}
					});
					touchedNodes = true;
					break;
				}
				case 'custom': {
					if (event.kind.topic === '__transport.resync_required') {
						needsSnapshotRefresh = true;
					}
					break;
				}
				case 'childAdded':
				case 'childRemoved':
				case 'childReplaced':
				case 'childMoved':
				case 'childReordered':
				case 'nodeCreated':
				case 'nodeDeleted':
					needsSnapshotRefresh = true;
					break;
			}
		}

		if (touchedNodes) {
			scopedNodesById = nextNodesById;
		}
		if (batch.to) {
			lastEventTime = batch.to;
		}
		if (needsSnapshotRefresh) {
			void refresh('batch');
		}
	};

	$effect(() => {
		const currentNodeId = selectedNodeId;
		if (currentNodeId === activeScopeNodeId && client === activeClientRef) {
			return;
		}
		activeScopeNodeId = currentNodeId;
		activeClientRef = client;

		const runId = activeRunId + 1;
		activeRunId = runId;

		stopActiveSubscription();
		scopedNodesById = new Map();
		scopedChildrenById = new Map();
		lastEventTime = undefined;
		loadError = null;
		isLoading = currentNodeId !== null;

		if (currentNodeId === null) {
			return;
		}

		const scope = inspectorScopeForNode(currentNodeId);
		let refreshInFlight = false;
		console.info(`[ui inspector] subscribe subtree root=${currentNodeId} depth=1`);

		const refresh = async (reason: string): Promise<void> => {
			if (refreshInFlight || runId !== activeRunId) {
				return;
			}
			refreshInFlight = true;
			try {
				await refreshScopeSnapshot(scope, runId, reason);
			} finally {
				refreshInFlight = false;
			}
		};

		void (async () => {
			try {
				const snapshot = await client.snapshot(scope);
				if (runId !== activeRunId) {
					return;
				}
				rebuildFromSnapshot(snapshot);
				isLoading = false;
				loadError = null;

				const unsubscribe = client.subscribe(scope, snapshot.at, (batch) => {
					applyScopedBatch(batch, runId, refresh);
				});
				activeUnsubscribe = () => {
					unsubscribe();
					console.info(
						`[ui inspector] unsubscribe subtree root=${currentNodeId} depth=1`
					);
				};
			} catch (error) {
				if (runId !== activeRunId) {
					return;
				}
				isLoading = false;
				loadError = error instanceof Error ? error.message : 'unknown inspector load error';
			}
		})();

	});

	onDestroy(() => {
		stopActiveSubscription();
		activeScopeNodeId = null;
		activeClientRef = null;
	});

	const dispatchEnableToggle = (node: UiNodeDto, enabled: boolean): void => {
		onIntent({
			kind: 'patchMeta',
			node: node.node_id,
			patch: { enabled }
		});
	};
</script>

<section class="inspector-panel">
	<header class="inspector-header">
		<h2>Inspector</h2>
		{#if lastEventTime}
			<p class="cursor">
				{lastEventTime.tick}:{lastEventTime.micro}:{lastEventTime.seq}
			</p>
		{/if}
	</header>

	{#if selectedNode}
		<div class="meta">
			<p class="label">{selectedNode.meta.label}</p>
			<p class="subtitle">{selectedNode.node_type}</p>
		</div>

		<div class="field">
			<label for="node-enabled">Enabled</label>
			<input
				id="node-enabled"
				type="checkbox"
				checked={selectedNode.meta.enabled}
				disabled={!selectedNode.meta.can_be_disabled}
				onchange={(event) =>
					dispatchEnableToggle(selectedNode, (event.currentTarget as HTMLInputElement).checked)}
			/>
		</div>

		{#if selectedNode.data.kind === 'parameter'}
			<ParameterInspector node={selectedNode} {onIntent} />
		{/if}

		{#if selectedNodeChildParameters.length > 0}
			<div class="param-list">
				<p class="param-list-title">Child Parameters</p>
				{#each selectedNodeChildParameters as childParam (childParam.node_id)}
					<ParameterInspector node={childParam} {onIntent} />
				{/each}
			</div>
		{:else if selectedNode.data.kind !== 'parameter'}
			<p class="empty">No direct parameter children.</p>
		{/if}

		{#if isLoading}
			<p class="hint">Updating scoped listener...</p>
		{/if}
		{#if loadError}
			<p class="error">Inspector sync error: {loadError}</p>
		{/if}
	{:else}
		<p class="empty">Select a node to inspect details.</p>
	{/if}
</section>

<style>
	.inspector-panel {
		background: var(--panel-bg);
		border: 1px solid var(--panel-border);
		border-radius: 14px;
		padding: 0.85rem;
		display: grid;
		gap: 0.7rem;
	}

	.inspector-header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.6rem;
	}

	.inspector-header h2 {
		margin: 0;
		font-size: 0.92rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.cursor {
		margin: 0;
		font-size: 0.7rem;
		opacity: 0.7;
	}

	.meta {
		margin-bottom: 0.35rem;
	}

	.label {
		margin: 0;
		font-size: 1.05rem;
		font-weight: 700;
	}

	.subtitle {
		margin: 0.15rem 0 0;
		font-size: 0.78rem;
		letter-spacing: 0.06em;
		text-transform: uppercase;
		opacity: 0.65;
	}

	.field {
		display: grid;
		grid-template-columns: 1fr;
		gap: 0.35rem;
	}

	.field label {
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	.param-list {
		display: grid;
		gap: 0.5rem;
	}

	.param-list-title {
		margin: 0;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	.hint {
		margin: 0;
		font-size: 0.75rem;
		opacity: 0.7;
	}

	.error {
		margin: 0;
		font-size: 0.78rem;
		color: #f5793b;
	}

	.empty {
		margin: 0;
		opacity: 0.75;
	}
</style>
