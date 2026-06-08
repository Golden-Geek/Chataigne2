<script lang="ts">
	import type { ProcessorStore } from '../stores/processorStore.svelte';

	let { store }: { store: ProcessorStore } = $props();
	let diagnostics = $derived([...store.diagnosticsById.values()]);
</script>

<ul aria-label="Diagnostics">
	{#each diagnostics as diagnostic (diagnostic.id)}
		<li class={diagnostic.severity}>
			<strong>{diagnostic.severity}</strong>
			<span>{diagnostic.message}</span>
		</li>
	{/each}
</ul>

<style>
	ul {
		display: grid;
		gap: 0.4rem;
		margin: 0;
		padding: 0.75rem;
		list-style: none;
	}
	li {
		display: flex;
		gap: 0.75rem;
		padding: 0.55rem;
		border-inline-start: 0.2rem solid #777;
	}
	li.error {
		border-color: #d75b5b;
	}
	li.warning {
		border-color: #d8a84e;
	}
</style>
