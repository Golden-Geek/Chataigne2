<script lang="ts">
	import type { RuntimeDebugStore } from '../stores/runtimeDebugStore.svelte';

	let { store }: { store: RuntimeDebugStore } = $props();
	let samples = $derived([...store.samplesByNode.values()].flatMap((values) => values.slice(-1)));
</script>

<aside>
	<header>
		<strong>Runtime Debug</strong>
		<button onclick={() => store.clear()}>Clear</button>
	</header>
	{#each samples as sample (`${sample.node_id}:${sample.socket_id ?? ''}`)}
		<div>
			<code>{sample.node_id}</code>
			<span>{sample.value}</span>
			<small>#{sample.execution_count}</small>
		</div>
	{/each}
</aside>

<style>
	aside {
		display: grid;
		gap: 0.4rem;
		padding: 0.75rem;
		background: color-mix(in srgb, #111 88%, transparent);
	}
	header,
	div {
		display: flex;
		justify-content: space-between;
		gap: 0.75rem;
	}
	button {
		font: inherit;
	}
</style>
