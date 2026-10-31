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
		configure(definition: ChannelDefinition): Promise<void>;
	} = $props();
	let width = $state<number | undefined>();
	let height = $state<number | undefined>();
	$effect(() => {
		const source = channel.definition.source;
		width = source.kind === 'Composition' ? source.canvas.width : undefined;
		height = source.kind === 'Composition' ? source.canvas.height : undefined;
	});
</script>

{#if width !== undefined && height !== undefined}
	<ChannelSettingsForm
		{channel}
		{disabled}
		{configure}
		title="Composition"
		source={{ kind: 'Composition', canvas: { width, height } }}>
		<p>
			Instance positions use canvas texels. Fixed output size changes resolution while preserving
			the canvas layout.
		</p>
		<label>Canvas width<input type="number" min="1" step="1" required bind:value={width} /></label>
		<label
			>Canvas height<input type="number" min="1" step="1" required bind:value={height} /></label>
	</ChannelSettingsForm>
{/if}
