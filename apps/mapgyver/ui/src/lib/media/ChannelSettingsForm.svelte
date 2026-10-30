<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import type { ChannelDefinition } from './generated/ChannelDefinition';
	import type { ChannelSource } from './generated/ChannelSource';
	import type { SampleFilter } from './generated/SampleFilter';

	let {
		channel,
		source,
		title,
		disabled,
		configure,
		children
	}: {
		channel: ChannelSnapshot;
		source: ChannelSource;
		title: string;
		disabled: boolean;
		configure: (definition: ChannelDefinition) => Promise<void>;
		children: Snippet;
	} = $props();
	let fixed = $state(false);
	let width = $state<number | undefined>();
	let height = $state<number | undefined>();
	let filter = $state<SampleFilter>();
	$effect(() => {
		const definition = channel.definition;
		fixed = definition.size.kind === 'Fixed';
		width = definition.size.kind === 'Fixed' ? definition.size.extent.width : undefined;
		height = definition.size.kind === 'Fixed' ? definition.size.extent.height : undefined;
		filter = definition.filter;
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!filter || (fixed && (width === undefined || height === undefined))) return;
		await configure({
			...channel.definition,
			source,
			size: fixed
				? { kind: 'Fixed', extent: { width: width!, height: height! } }
				: { kind: 'Automatic' },
			filter
		});
	}
</script>

<form onsubmit={submit}>
	<fieldset {disabled}>
		<legend>{title} · {channel.label}</legend>
		{@render children()}
		<label class="toggle"><input type="checkbox" bind:checked={fixed} /> Fixed output size</label>
		{#if fixed}
			<div class="dimensions">
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
		max-width: 100%;
		overflow-wrap: anywhere;
	}
	fieldset :global(label) {
		display: grid;
		gap: 0.35em;
		min-width: 0;
	}
	.dimensions {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(5rem, 1fr));
		gap: 0.7rem;
	}
	fieldset :global(input),
	fieldset :global(select),
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
	label.toggle {
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
