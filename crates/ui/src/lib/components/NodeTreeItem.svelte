<script lang="ts">
	import NodeTreeItem from './NodeTreeItem.svelte';
	import type { GraphState } from '../store/graph';
	import type { NodeId } from '../types';

	let {
		nodeId,
		state,
		depth = 0,
		selectedNodeId = null,
		onSelect
	}: {
		nodeId: NodeId;
		state: GraphState;
		depth?: number;
		selectedNodeId?: NodeId | null;
		onSelect: (nodeId: NodeId) => void;
	} = $props();

	const node = $derived(state.nodesById.get(nodeId));
	const children = $derived(state.childrenById.get(nodeId) ?? []);
	const isSelected = $derived(nodeId === selectedNodeId);
</script>

{#if node}
	<li class="tree-row" style={`--depth:${depth};`}>
		<button
			type="button"
			class:selected={isSelected}
			class="tree-button"
			onclick={() => onSelect(nodeId)}
		>
			<span class="tree-label">{node.meta.label}</span>
			<span class="tree-type">{node.node_type}</span>
		</button>
	</li>
	{#if children.length > 0}
		<ul class="tree-children">
			{#each children as childId (childId)}
				<NodeTreeItem {state} nodeId={childId} depth={depth + 1} {selectedNodeId} {onSelect} />
			{/each}
		</ul>
	{/if}
{/if}

<style>
	.tree-row {
		list-style: none;
	}

	.tree-button {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		width: 100%;
		border: none;
		border-left: 2px solid transparent;
		background: transparent;
		padding: 0.35rem 0.5rem 0.35rem calc(0.5rem + var(--depth) * 1rem);
		text-align: left;
		color: inherit;
		cursor: pointer;
	}

	.tree-button:hover {
		background: color-mix(in srgb, var(--panel-bg) 82%, white 18%);
	}

	.tree-button.selected {
		border-left-color: var(--accent);
		background: color-mix(in srgb, var(--panel-bg) 72%, var(--accent) 28%);
	}

	.tree-label {
		font-weight: 600;
	}

	.tree-type {
		font-size: 0.72rem;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		opacity: 0.65;
	}

	.tree-children {
		margin: 0;
		padding: 0;
	}
</style>
