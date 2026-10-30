<script lang="ts">
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import type { SampleFilter } from './generated/SampleFilter';

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
	let fixed = $state(false);
	let width = $state<number | undefined>();
	let height = $state<number | undefined>();
	let filter = $state<SampleFilter>('Linear');

	$effect(() => {
		const definition = channel.definition;
		rgba = definition.source.kind === 'SolidColor' ? [...definition.source.rgba] : [];
		fixed = definition.size.kind === 'Fixed';
		width = definition.size.kind === 'Fixed' ? definition.size.extent.width : undefined;
		height = definition.size.kind === 'Fixed' ? definition.size.extent.height : undefined;
		filter = definition.filter;
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (fixed && (width === undefined || height === undefined)) return;
		await configure({
			...channel.definition,
			source: { kind: 'SolidColor', rgba: [rgba[0], rgba[1], rgba[2], rgba[3]] },
			size: fixed
				? { kind: 'Fixed', extent: { width: width!, height: height! } }
				: { kind: 'Automatic' },
			filter
		});
	}
</script>

<form onsubmit={submit}>
	<fieldset {disabled}>
		<legend>Solid Color · {channel.label}</legend>
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
		<label class="toggle"><input type="checkbox" bind:checked={fixed} /> Fixed output size</label>
		{#if fixed}
			<div class="components">
				<label
					>Width (texels)<input
						type="number"
						min="1"
						step="1"
						required
						bind:value={width} /></label>
				<label
					>Height (texels)<input
						type="number"
						min="1"
						step="1"
						required
						bind:value={height} /></label>
			</div>
		{:else}
			<p class="hint">Automatic size follows the source and its consumers.</p>
		{/if}
		<label
			>Sampling<select bind:value={filter}
				><option value="Linear">Linear</option><option value="Nearest">Nearest</option></select
			></label>
		<button type="submit">Apply channel settings</button>
	</fieldset>
</form>

<style>
	fieldset {
		display: grid;
		gap: 1rem;
		border: 0.0625rem solid #ffffff24;
		border-radius: 0.5rem;
		padding: 1rem;
		min-width: 0;
	}
	legend {
		padding: 0 0.4em;
		font-weight: 600;
	}
	label {
		display: grid;
		gap: 0.35em;
		min-width: 0;
	}
	.components {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(5rem, 1fr));
		gap: 0.7rem;
	}
	input,
	select,
	button {
		font: inherit;
		color: inherit;
		background: #ffffff0a;
		border: 0.0625rem solid #ffffff30;
		border-radius: 0.3rem;
		padding: 0.5em 0.65em;
		min-width: 0;
	}
	button {
		justify-self: start;
		cursor: pointer;
		background: #416caa50;
	}
	.toggle {
		display: flex;
		align-items: center;
		gap: 0.5em;
	}
	.hint {
		margin: 0;
		opacity: 0.65;
		font-size: 0.9em;
	}
</style>
