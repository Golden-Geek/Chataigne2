<script lang="ts">
	import type { StatechartStore } from '../stores/statechartStore.svelte';

	let { store }: { store: StatechartStore } = $props();
	let states = $derived([...store.statesById.values()]);
</script>

<div class="canvas" role="tree" aria-label="Statechart">
	{#each states as state (state.id)}
		<button
			class:active={store.activeStateIds.has(state.id)}
			class:selected={store.selectedStateId === state.id}
			style:left={`${state.layout.x}rem`}
			style:top={`${state.layout.y}rem`}
			onclick={() => store.select(state.id)}>
			<strong>{state.label}</strong>
			<small>{state.kind}</small>
		</button>
	{/each}
</div>

<style>
	.canvas {
		position: relative;
		min-height: 28rem;
		overflow: auto;
		background: color-mix(in srgb, var(--background, #171717) 92%, white);
	}
	button {
		position: absolute;
		display: grid;
		min-width: 8rem;
		gap: 0.25rem;
		padding: 0.75rem 1rem;
		border: 0.08rem solid #555;
		border-radius: 0.5rem;
		background: #242424;
		color: inherit;
		text-align: left;
	}
	button.active {
		border-color: #65c98b;
	}
	button.selected {
		outline: 0.15rem solid #7aa7ff;
	}
	small {
		opacity: 0.65;
	}
</style>
