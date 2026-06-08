<script lang="ts">
	import type { AlchemistGraphStore } from '../stores/alchemistGraphStore.svelte';

	let { store }: { store: AlchemistGraphStore } = $props();
	let viewport = $state({ x: 0, y: 0, width: 80, height: 45 });
	let nodes = $derived(store.visibleNodes(viewport));
</script>

<div class="graph">
	{#each nodes as node (node.id)}
		<button
			class:selected={store.selectedNodeIds.has(node.id)}
			style:left={`${node.x}rem`}
			style:top={`${node.y}rem`}
			onclick={() => {
				store.selectedNodeIds.clear();
				store.selectedNodeIds.add(node.id);
			}}>
			<strong>{node.label}</strong>
			<small>{node.type_id}</small>
		</button>
	{/each}
</div>

<style>
	.graph {
		position: relative;
		min-height: 32rem;
		overflow: hidden;
		background-size: 1rem 1rem;
		background-image: radial-gradient(#444 0.06rem, transparent 0.06rem);
	}
	button {
		position: absolute;
		display: grid;
		min-width: 9rem;
		gap: 0.35rem;
		padding: 0.65rem;
		border: 0.08rem solid #555;
		border-radius: 0.4rem;
		background: #202020;
		color: inherit;
		text-align: left;
	}
	button.selected {
		border-color: #7aa7ff;
	}
	small {
		opacity: 0.6;
	}
</style>
