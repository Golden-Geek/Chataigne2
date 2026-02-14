<script lang="ts">
	import NodeTreeItem from './NodeTreeItem.svelte';
	import type { GraphState } from '../store/graph';
	import type { NodeId } from '../types';

	let {
		state,
		selectedNodeId = null,
		onSelect
	}: {
		state: GraphState;
		selectedNodeId?: NodeId | null;
		onSelect: (nodeId: NodeId) => void;
	} = $props();

	const rootId = $derived(state.rootId);
</script>

<section class="tree-panel">
	<header class="tree-header">
		<h2>Graph</h2>
		{#if state.requiresResync}
			<span class="sync-flag">Resync needed</span>
		{/if}
	</header>
	{#if rootId !== null}
		<ul class="tree-root">
			<NodeTreeItem nodeId={rootId} {state} depth={0} {selectedNodeId} {onSelect} />
		</ul>
	{:else}
		<p class="empty">No nodes available.</p>
	{/if}
</section>

<style>
	.tree-panel {
		background: var(--panel-bg);
		border: 1px solid var(--panel-border);
		border-radius: 14px;
		padding: 0.65rem;
	}

	.tree-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.75rem;
		padding: 0.35rem 0.4rem 0.6rem 0.4rem;
	}

	.tree-header h2 {
		margin: 0;
		font-size: 0.92rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.sync-flag {
		font-size: 0.7rem;
		color: #f5793b;
		font-weight: 700;
		text-transform: uppercase;
		letter-spacing: 0.06em;
	}

	.tree-root {
		margin: 0;
		padding: 0;
	}

	.empty {
		margin: 0;
		padding: 0.8rem;
		opacity: 0.75;
	}
</style>
