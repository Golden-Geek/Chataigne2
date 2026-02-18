<script lang="ts">
	import ParameterInspector from './ParameterInspector.svelte';
	import { getWorkbenchContext } from '../store/workbench-context';
	import type { UiNodeDto } from '../types';

	const session = getWorkbenchContext();
	const graph = $derived(session.graph.state);

	const selectedNode = $derived.by(() => {
		const nodeId = session.selectedNodeId;
		if (nodeId === null) {
			return null;
		}
		return graph.nodesById.get(nodeId) ?? null;
	});

	const selectedNodeChildren = $derived.by(() => {
		if (!selectedNode) {
			return [];
		}
		const childIds = graph.childrenById.get(selectedNode.node_id) ?? [];
		return childIds
			.map((childId) => graph.nodesById.get(childId))
			.filter((node): node is UiNodeDto => node !== undefined);
	});

	const selectedNodeParentId = $derived.by(() => {
		if (!selectedNode) {
			return null;
		}
		for (const [parentId, children] of graph.childrenById.entries()) {
			if (children.includes(selectedNode.node_id)) {
				return parentId;
			}
		}
		return null;
	});

	const selectedNodeSiblingIds = $derived.by(() => {
		if (selectedNodeParentId === null) {
			return [];
		}
		return graph.childrenById.get(selectedNodeParentId) ?? [];
	});

	const selectedNodeParent = $derived.by(() => {
		if (selectedNodeParentId === null) {
			return null;
		}
		return graph.nodesById.get(selectedNodeParentId) ?? null;
	});

	const selectedNodeSiblingIndex = $derived.by(() => {
		if (!selectedNode) {
			return -1;
		}
		return selectedNodeSiblingIds.indexOf(selectedNode.node_id);
	});

	const canEditSelectedManagerFolder = $derived(
		selectedNode !== null &&
			selectedNode.node_type === 'folder' &&
			selectedNodeParent !== null &&
			selectedNodeParent.accepted_user_item_kinds.length > 0
	);
	const canEditSelectedUserItem = $derived(
		selectedNode !== null &&
			selectedNodeParentId !== null &&
			(selectedNode.user_role === 'itemRoot' || canEditSelectedManagerFolder)
	);
	const canMoveSelectedUserItemUp = $derived(canEditSelectedUserItem && selectedNodeSiblingIndex > 0);
	const canMoveSelectedUserItemDown = $derived(canEditSelectedUserItem && selectedNodeSiblingIndex >= 0 && selectedNodeSiblingIndex < selectedNodeSiblingIds.length - 1);

	const selectedNodeChildParameters = $derived.by(() =>
		selectedNodeChildren.filter(
			(node): node is UiNodeDto =>
				node.data.kind === 'parameter'
		)
	);

	const selectedNodeCreatableItems = $derived(selectedNode ? selectedNode.creatable_user_items : []);

	let selectedCreatableNodeType = $state('');
	let createLabel = $state('');

	$effect(() => {
		const items = selectedNodeCreatableItems;
		if (items.length === 0) {
			selectedCreatableNodeType = '';
			createLabel = '';
			return;
		}
		if (!items.some((item) => item.node_type === selectedCreatableNodeType)) {
			selectedCreatableNodeType = items[0].node_type;
			createLabel = items[0].label;
		}
	});

	const selectedCreatableItem = $derived.by(() =>
		selectedNodeCreatableItems.find((item) => item.node_type === selectedCreatableNodeType) ?? null
	);

	const dispatchEnableToggle = (node: UiNodeDto, enabled: boolean): void => {
		void session.sendIntent({
			kind: 'patchMeta',
			node: node.node_id,
			patch: { enabled }
		});
	};

	const dispatchCreateUserItem = (node: UiNodeDto): void => {
		if (!selectedCreatableItem) {
			return;
		}
		const trimmed = createLabel.trim();
		void session.sendIntent({
			kind: 'createUserItem',
			parent: node.node_id,
			node_type: selectedCreatableItem.node_type,
			label: trimmed.length > 0 ? trimmed : selectedCreatableItem.label
		});
	};

	const dispatchDeleteSelectedUserItem = (): void => {
		if (!selectedNode || !canEditSelectedUserItem) {
			return;
		}
		void session.sendIntent({
			kind: 'removeNode',
			node: selectedNode.node_id
		});
	};

	const dispatchMoveSelectedUserItemUp = (): void => {
		if (
			!selectedNode ||
			selectedNodeParentId === null ||
			!canMoveSelectedUserItemUp
		) {
			return;
		}
		const previousSiblingId = selectedNodeSiblingIds[selectedNodeSiblingIndex - 1];
		if (previousSiblingId === undefined) {
			return;
		}
		void session.sendIntent({
			kind: 'moveNode',
			node: previousSiblingId,
			new_parent: selectedNodeParentId,
			new_prev_sibling: selectedNode.node_id
		});
	};

	const dispatchMoveSelectedUserItemDown = (): void => {
		if (
			!selectedNode ||
			selectedNodeParentId === null ||
			!canMoveSelectedUserItemDown
		) {
			return;
		}
		const nextSiblingId = selectedNodeSiblingIds[selectedNodeSiblingIndex + 1];
		if (nextSiblingId === undefined) {
			return;
		}
		void session.sendIntent({
			kind: 'moveNode',
			node: selectedNode.node_id,
			new_parent: selectedNodeParentId,
			new_prev_sibling: nextSiblingId
		});
	};

	const formatNodeCursor = (time: { tick: number; micro: number; seq: number }): string =>
		`${time.tick}:${time.micro}:${time.seq}`;
</script>

<section class="inspector-panel">
	<header class="inspector-header">
		<h2>Inspector</h2>
		{#if graph.lastEventTime}
			<p class="cursor">
				{formatNodeCursor(graph.lastEventTime)}
			</p>
		{/if}
	</header>

	{#if selectedNode}
		<div class="meta">
			<p class="label">{selectedNode.meta.label}</p>
			<p class="subtitle">{selectedNode.node_type}</p>
		</div>

		{#if selectedNode.meta.can_be_disabled !== false}
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
		{/if}
		{#if selectedNode.data.kind === 'parameter'}
			<ParameterInspector node={selectedNode} />
		{/if}

		{#if canEditSelectedUserItem}
			<div class="item-actions">
				<p class="item-actions-title">Item Actions</p>
				<div class="item-actions-row">
					<button
						type="button"
						class="secondary-action"
						disabled={!canMoveSelectedUserItemUp}
						onclick={dispatchMoveSelectedUserItemUp}
					>
						Move Up
					</button>
					<button
						type="button"
						class="secondary-action"
						disabled={!canMoveSelectedUserItemDown}
						onclick={dispatchMoveSelectedUserItemDown}
					>
						Move Down
					</button>
					<button
						type="button"
						class="danger-action"
						onclick={dispatchDeleteSelectedUserItem}
					>
						Delete
					</button>
				</div>
			</div>
		{/if}

		{#if selectedNodeCreatableItems.length > 0}
			<div class="create-panel">
				<p class="create-title">Add Item</p>
				<div class="field">
					<label for="create-item-type">Type</label>
					<select
						id="create-item-type"
						bind:value={selectedCreatableNodeType}
						onchange={() => {
							if (selectedCreatableItem) {
								createLabel = selectedCreatableItem.label;
							}
						}}
					>
						{#each selectedNodeCreatableItems as item (item.node_type)}
							<option value={item.node_type}>{item.label} ({item.node_type})</option>
						{/each}
					</select>
				</div>
				<div class="field">
					<label for="create-item-label">Label</label>
					<input id="create-item-label" type="text" bind:value={createLabel} />
				</div>
				<button type="button" class="create-button" onclick={() => dispatchCreateUserItem(selectedNode)}>
					Add
				</button>
			</div>
		{/if}

		{#if selectedNodeChildParameters.length > 0}
			<div class="param-list">
				<p class="param-list-title">Child Parameters</p>
				{#each selectedNodeChildParameters as childParam (childParam.node_id)}
					<ParameterInspector node={childParam} />
				{/each}
			</div>
		{:else if selectedNode.data.kind !== 'parameter'}
			<p class="empty">No direct parameter children.</p>
		{/if}
	{:else}
		<p class="empty">Select a node to inspect details.</p>
	{/if}
</section>

<style>
	.inspector-panel {
		background: var(--panel-bg);
		border: 0.0625rem solid var(--panel-border);
		border-radius: 0.875rem;
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

	.create-panel {
		display: grid;
		gap: 0.45rem;
		padding: 0.55rem;
		border: 0.0625rem solid color-mix(in srgb, var(--panel-border) 84%, white 16%);
		border-radius: 0.625rem;
	}

	.item-actions {
		display: grid;
		gap: 0.45rem;
		padding: 0.55rem;
		border: 0.0625rem solid color-mix(in srgb, var(--panel-border) 84%, white 16%);
		border-radius: 0.625rem;
	}

	.item-actions-title {
		margin: 0;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	.item-actions-row {
		display: flex;
		gap: 0.45rem;
		flex-wrap: wrap;
	}

	.create-title {
		margin: 0;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	.param-list-title {
		margin: 0;
		font-size: 0.75rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.75;
	}

	select,
	input[type='text'] {
		background: color-mix(in srgb, var(--panel-bg) 82%, white 18%);
		color: inherit;
		border: 0.0625rem solid color-mix(in srgb, var(--panel-border) 84%, white 16%);
		border-radius: 0.5rem;
		padding: 0.35rem 0.45rem;
	}

	.create-button {
		border: none;
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--accent) 72%, black 28%);
		color: #fff;
		font-weight: 700;
		letter-spacing: 0.04em;
		padding: 0.45rem 0.6rem;
		cursor: pointer;
	}

	.create-button:hover {
		background: color-mix(in srgb, var(--accent) 82%, black 18%);
	}

	.secondary-action,
	.danger-action {
		border: none;
		border-radius: 0.5rem;
		color: #fff;
		font-weight: 700;
		letter-spacing: 0.04em;
		padding: 0.4rem 0.55rem;
		cursor: pointer;
	}

	.secondary-action {
		background: color-mix(in srgb, var(--panel-bg) 55%, white 45%);
	}

	.secondary-action:hover:not(:disabled) {
		background: color-mix(in srgb, var(--panel-bg) 45%, white 55%);
	}

	.danger-action {
		background: color-mix(in srgb, #c63b1f 74%, black 26%);
	}

	.danger-action:hover {
		background: color-mix(in srgb, #c63b1f 84%, black 16%);
	}

	.secondary-action:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.empty {
		margin: 0;
		opacity: 0.75;
	}
</style>
