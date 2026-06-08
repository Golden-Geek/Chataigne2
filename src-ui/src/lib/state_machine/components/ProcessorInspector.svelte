<script lang="ts">
	import type { ProcessorStore } from '../stores/processorStore.svelte';

	let { store }: { store: ProcessorStore } = $props();
	let processor = $derived(store.selected);
</script>

<section>
	{#if processor}
		<header>
			<h2>{processor.label}</h2>
			<span>{processor.active ? 'Active' : 'Inactive'}</span>
		</header>
		{#each processor.exposed as declaration (declaration.id)}
			<label>
				<span>{declaration.label}</span>
				<output>{declaration.value_type}</output>
			</label>
		{/each}
	{:else}
		<p>Select a Processor.</p>
	{/if}
</section>

<style>
	section {
		display: grid;
		gap: 0.75rem;
		padding: 1rem;
	}
	header,
	label {
		display: flex;
		justify-content: space-between;
		gap: 1rem;
	}
	h2 {
		margin: 0;
		font-size: 1.1em;
	}
</style>
