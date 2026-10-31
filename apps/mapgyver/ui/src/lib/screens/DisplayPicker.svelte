<script lang="ts">
	import { onMount } from 'svelte';
	import type { DisplayCatalogSnapshot } from './generated/DisplayCatalogSnapshot';
	import { createDisplayCatalogTransport, type DisplayCatalogTransport } from './display-discovery';
	let {
		display = $bindable(),
		disabled,
		transport = createDisplayCatalogTransport()
	}: { display: string; disabled: boolean; transport?: DisplayCatalogTransport } = $props();
	let catalog = $state<DisplayCatalogSnapshot | null>(null);
	let connected = $state(false);
	let error = $state('');
	let stopped = false;
	const available = $derived(transport.available());
	function accept(next: DisplayCatalogSnapshot) {
		if (!stopped && (!catalog || next.revision >= catalog.revision)) catalog = next;
	}
	async function refresh() {
		if (!connected || stopped) return;
		try {
			accept(await transport.request(true));
			if (!stopped) error = '';
		} catch (cause) {
			if (!stopped) error = String(cause);
		}
	}
	onMount(() => {
		let unlisten: (() => void) | undefined;
		if (available)
			void transport
				.subscribe(accept)
				.then((dispose) => {
					if (stopped) {
						dispose();
						return;
					}
					unlisten = dispose;
					connected = true;
					void refresh();
				})
				.catch((cause) => {
					if (!stopped) error = String(cause);
				});
		return () => {
			stopped = true;
			unlisten?.();
		};
	});
</script>

<div class="picker">
	{#if available}
		<label
			>Fullscreen display<select bind:value={display} disabled={disabled || !catalog}>
				<option value="">Choose a display</option>
				{#if display && !catalog?.displays.some((entry) => entry.id === display)}
					<option value={display}>Selected display unavailable (selection retained)</option>
				{/if}
				{#each catalog?.displays ?? [] as entry (entry.id)}
					<option value={entry.id}
						>{entry.name || 'Unnamed display'} · {entry.bounds.extent.width}×{entry.bounds.extent
							.height} at {entry.bounds.left},{entry.bounds.top}</option>
				{/each}
			</select></label>
		<button type="button" disabled={disabled || !connected} onclick={refresh}
			>Refresh displays</button>
		<p role="status">
			{error ||
				catalog?.error ||
				(catalog
					? `${catalog.displays.length} active display(s)`
					: 'Connecting to display discovery…')}
		</p>
	{:else}
		<label>Intended display identity<input bind:value={display} {disabled} required /></label>
		<p>Display discovery is available in the desktop app. Unavailable selections are retained.</p>
	{/if}
	<p>Fullscreen output stays inactive while its selected display is unavailable.</p>
</div>

<style>
	.picker {
		display: grid;
		gap: 0.45rem;
	}
	label {
		display: grid;
		gap: 0.3rem;
	}
	select,
	input {
		min-width: 0;
		max-width: 100%;
	}
	p {
		margin: 0;
		opacity: 0.75;
		font-size: 0.85em;
	}
</style>
