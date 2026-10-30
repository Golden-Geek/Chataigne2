<script lang="ts">
	import { onMount } from 'svelte';
	import type { SenderDiscoverySnapshot } from './generated/SenderDiscoverySnapshot';
	import {
		createSenderDiscoveryTransport,
		type SenderDiscoveryTransport
	} from './sender-discovery';
	import { newerDiscovery } from './discovery-state';

	let {
		sender = $bindable(),
		disabled,
		transport = createSenderDiscoveryTransport()
	}: { sender: string; disabled: boolean; transport?: SenderDiscoveryTransport } = $props();
	let catalog = $state<SenderDiscoverySnapshot | null>(null);
	let connected = $state(false);
	let error = $state('');
	let stopped = false;
	const available = $derived(transport.available());
	function accept(snapshot: SenderDiscoverySnapshot) {
		if (!stopped) catalog = newerDiscovery(catalog, snapshot);
	}
	async function refresh() {
		if (!connected || stopped) return;
		error = '';
		try {
			accept(await transport.request(true));
		} catch (cause) {
			if (!stopped) error = String(cause);
		}
	}
	onMount(() => {
		let unlisten: (() => void) | undefined;
		if (available) {
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
		}
		return () => {
			stopped = true;
			unlisten?.();
		};
	});
</script>

<div class="picker">
	<label>Spout sender name<input type="text" bind:value={sender} {disabled} /></label>
	{#if available}
		<label
			>Discovered senders<select bind:value={sender} disabled={disabled || !catalog}>
				<option value="">Unconfigured</option>
				{#if sender && !catalog?.names.includes(sender)}
					<option value={sender}>{sender} (not in last scan)</option>
				{/if}
				{#each catalog?.names ?? [] as name (name)}<option value={name}>{name}</option>{/each}
			</select></label>
		<button
			type="button"
			disabled={disabled || !connected || catalog?.refreshing}
			onclick={refresh}>
			{catalog?.refreshing ? 'Finding senders…' : 'Refresh senders'}
		</button>
		<p class="hint" role="status">
			{#if error || catalog?.error}{error || catalog?.error}
			{:else if catalog?.refreshing}Scanning for Spout senders…
			{:else if catalog}{catalog.names.length} sender{catalog.names.length === 1 ? '' : 's'} found.
			{:else}Connecting to sender discovery…{/if}
		</p>
		{#if catalog?.truncated}<p class="hint">
				The list is partial. You can enter an exact sender name above.
			</p>{/if}
	{:else}
		<p class="hint">Find available senders in the desktop app, or enter an exact name above.</p>
	{/if}
</div>

<style>
	.picker {
		display: grid;
		gap: 0.7rem;
		min-width: 0;
	}
	button {
		justify-self: start;
		padding: 0.5em 0.65em;
		font: inherit;
		color: inherit;
		background: #416caa50;
		border: 0.0625rem solid #ffffff30;
		border-radius: 0.3rem;
		cursor: pointer;
	}
	.hint {
		margin: 0;
		opacity: 0.65;
		font-size: 0.9em;
		overflow-wrap: anywhere;
	}
</style>
