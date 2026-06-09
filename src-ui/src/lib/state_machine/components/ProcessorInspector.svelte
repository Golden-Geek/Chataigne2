<script lang="ts">
	import type { ProcessorStore } from '../stores/processorStore.svelte';

	let { store }: { store: ProcessorStore } = $props();
	let processor = $derived(store.selected);
</script>

<section>
	{#if processor}
		<header>
			<div>
				<h2>{processor.label}</h2>
				<small>{processor.formula_family.replace('_', ' ')}</small>
			</div>
			<span>{processor.active ? 'Active' : 'Inactive'}</span>
		</header>
		{#each processor.surface as surfaceSection (surfaceSection.id)}
			<fieldset>
				<legend>{surfaceSection.label}</legend>
				{#if surfaceSection.items.length === 0}
					<p class="empty">None</p>
				{:else}
					{#each surfaceSection.items as item (item.id)}
						<label>
							<span>{item.label}</span>
							<output>{item.value_type ?? item.kind}</output>
						</label>
					{/each}
				{/if}
			</fieldset>
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
	fieldset {
		display: grid;
		gap: 0.5rem;
		margin: 0;
		padding: 0.75rem;
		border: solid 0.06rem var(--gc-color-panel-outline);
		border-radius: 0.35rem;
	}
	legend {
		padding-inline: 0.25rem;
		font-weight: 600;
	}
	h2 {
		margin: 0;
		font-size: 1.1em;
	}
	small,
	.empty {
		color: var(--gc-color-text-muted);
		text-transform: capitalize;
	}
	.empty {
		margin: 0;
	}
</style>
