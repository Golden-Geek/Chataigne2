<script lang="ts">
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import type { TextureDescriptor } from './generated/TextureDescriptor';
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
	let sender = $state('');
	let maximum = $state<TextureDescriptor>();
	$effect(() => {
		const source = channel.definition.source;
		if (source.kind !== 'Spout') return;
		sender = source.sender ?? '';
		maximum = { ...source.maximum, extent: { ...source.maximum.extent } };
	});
</script>

{#if maximum}
	<ChannelSettingsForm
		{channel}
		{disabled}
		{configure}
		title="Shared Texture"
		source={{ kind: 'Spout', sender: sender === '' ? null : sender, maximum }}>
		<label>Spout sender name<input type="text" bind:value={sender} /></label>
		<p class="hint">
			Use the exact sender name. Leave empty to disconnect this channel. The receiver reconnects to
			that name when it becomes available; other channels keep their own selections.
		</p>
		<fieldset>
			<legend>Accepted source</legend>
			<p class="hint">
				Frames must fit these maximum dimensions and match the selected storage, color and alpha.
				Output size is configured separately below.
			</p>
			<div class="dimensions">
				<label
					>Maximum width (texels)<input
						type="number"
						min="1"
						step="1"
						required
						bind:value={maximum.extent.width} /></label>
				<label
					>Maximum height (texels)<input
						type="number"
						min="1"
						step="1"
						required
						bind:value={maximum.extent.height} /></label>
			</div>
			<label
				>Storage<select bind:value={maximum.format}>
					<option value="Bgra8Unorm">BGRA8</option>
					<option value="Rgba8Unorm">RGBA8</option>
				</select></label>
			<label
				>Color encoding<select bind:value={maximum.color_encoding}>
					<option value="Srgb">sRGB</option>
					<option value="Linear">Linear</option>
				</select></label>
			<label
				>Source alpha<select bind:value={maximum.alpha_mode}>
					<option value="Straight">Straight</option>
					<option value="Premultiplied">Premultiplied</option>
					<option value="Opaque">Opaque</option>
				</select></label>
		</fieldset>
	</ChannelSettingsForm>
{/if}

<style>
	fieldset {
		display: grid;
		gap: 0.8rem;
		min-width: 0;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.4rem;
		padding: 0.8rem;
	}
	.dimensions {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(6rem, 1fr));
		gap: 0.7rem;
	}
	.hint {
		margin: 0;
		opacity: 0.65;
		font-size: 0.9em;
	}
</style>
