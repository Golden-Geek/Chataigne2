<script lang="ts">
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import type { AssetSnapshot } from './generated/AssetSnapshot';
	import type { MediaEdit } from './generated/MediaEdit';
	import type { MediaEditResult } from './generated/MediaEditResult';
	import ChannelSettingsForm from './ChannelSettingsForm.svelte';
	import AssetLocatorForm from './AssetLocatorForm.svelte';

	let {
		channel,
		assets,
		disabled,
		configure,
		edit
	}: {
		channel: ChannelSnapshot;
		assets: AssetSnapshot[];
		disabled: boolean;
		configure: (definition: ChannelDefinition) => Promise<void>;
		edit: (intent: MediaEdit) => Promise<MediaEditResult | undefined>;
	} = $props();
	let asset = $state('');
	const record = $derived(assets.find((entry) => entry.id === asset));
	$effect(() => {
		asset =
			channel.definition.source.kind === 'Picture' ? (channel.definition.source.asset ?? '') : '';
	});
</script>

<ChannelSettingsForm
	{channel}
	{disabled}
	{configure}
	title="Picture"
	source={{ kind: 'Picture', asset: asset || null }}>
	<label
		>Picture asset<select bind:value={asset}>
			<option value="">Unconfigured</option>
			{#if asset && !record}<option value={asset}>Missing asset · {asset}</option>{/if}
			{#each assets as entry (entry.id)}<option value={entry.id}>{entry.label}</option>{/each}
		</select></label>
	{#if record?.locator}<p class="path">{record.locator.path}</p>{/if}
</ChannelSettingsForm>

<section aria-label="Import a Picture for this channel">
	<h3>Import and assign</h3>
	<AssetLocatorForm
		{disabled}
		submitLabel="Import and assign"
		submit={(locator) =>
			edit({ kind: 'ImportPicture', locator, target: channel.reference, label: null })} />
</section>
{#if record}
	<section aria-label="Relink selected Picture asset">
		<h3>Relink {record.label}</h3>
		<p class="hint">This is a shared asset. Relinking updates every channel that uses it.</p>
		{#key record.id}
			<AssetLocatorForm
				{disabled}
				initial={record.locator}
				submitLabel="Relink asset"
				submit={(locator) => edit({ kind: 'RelinkAsset', asset: record!.id, locator })} />
		{/key}
	</section>
{/if}

<style>
	section {
		display: grid;
		gap: 0.7rem;
	}
	h3 {
		font-size: 1em;
		margin: 0;
	}
	.path {
		margin: 0;
		overflow-wrap: anywhere;
		opacity: 0.7;
	}
	.hint {
		margin: 0;
		font-size: 0.9em;
		opacity: 0.7;
	}
</style>
