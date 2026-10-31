<script lang="ts">
	import type { CatalogState } from './catalog';
	let {
		state,
		disabled,
		update
	}: {
		state: CatalogState;
		disabled: boolean;
		update(search: string, start: number, refresh: boolean): unknown;
	} = $props();
	function search() {
		update(state.search, 0, true);
	}
</script>

<div class="catalog-controls">
	<input
		aria-label="Find shared application"
		type="search"
		value={state.search}
		oninput={(event) => update(event.currentTarget.value, state.start, false)}
		{disabled}
		onkeydown={(event) => {
			if (event.key === 'Enter') {
				event.preventDefault();
				search();
			}
		}} />
	<button type="button" disabled={disabled || state.loading} onclick={search}>Search apps</button>
	<button
		type="button"
		disabled={disabled || state.loading || state.start === 0}
		onclick={() => {
			update(state.search, Math.max(0, state.start - 64), true);
		}}>Previous apps</button>
	<button
		type="button"
		disabled={disabled || state.loading || !state.page || state.start + 64 >= state.page.total}
		onclick={() => {
			update(state.search, state.start + 64, true);
		}}>Next apps</button>
	{#if state.page}<small
			>{state.page.total === 0 ? 0 : state.page.start + 1}–{state.page.start +
				state.page.applications.length} of {state.page.total}</small
		>{/if}
	{#if state.error}<p role="alert">{state.error}</p>{/if}
</div>

<style>
	.catalog-controls {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem;
	}
	input {
		min-width: 0;
		max-width: 100%;
	}
	p {
		flex-basis: 100%;
	}
</style>
