<script lang="ts">
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import ChannelSettingsForm from './ChannelSettingsForm.svelte';

	let {
		channel,
		disabled,
		configure
	}: {
		channel: ChannelSnapshot;
		disabled: boolean;
		configure: (definition: ChannelDefinition) => Promise<void>;
	} = $props();
	let rgba = $state<number[]>([]);
	$effect(() => {
		rgba =
			channel.definition.source.kind === 'SolidColor' ? [...channel.definition.source.rgba] : [];
	});
</script>

<ChannelSettingsForm
	{channel}
	{disabled}
	{configure}
	title="Solid Color"
	source={{ kind: 'SolidColor', rgba: [rgba[0], rgba[1], rgba[2], rgba[3]] }}>
	<p class="hint">Straight linear RGBA. Changes apply to this channel only.</p>
	<div class="components">
		{#each ['Red', 'Green', 'Blue', 'Alpha'] as label, index}
			<label
				>{label}<input
					aria-label={label}
					type="number"
					min="0"
					max="1"
					step="any"
					required
					bind:value={rgba[index]} /></label>
		{/each}
	</div>
</ChannelSettingsForm>

<style>
	.components {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(5rem, 1fr));
		gap: 0.7rem;
	}
	.hint {
		margin: 0;
		opacity: 0.65;
		font-size: 0.9em;
	}
</style>
