<script lang="ts">
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import type { SurfaceDefinition } from './generated/SurfaceDefinition';
	import type { ChannelReference } from './generated/ChannelReference';
	import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';
	import { channelKey } from '../media/identity';

	let {
		surface,
		media,
		disabled,
		configure
	}: {
		surface: SurfaceSnapshot;
		media: MediaManagerSnapshot | null;
		disabled: boolean;
		configure: (definition: SurfaceDefinition) => Promise<unknown>;
	} = $props();
	let draft = $state<SurfaceDefinition>();
	let source = $state<ChannelReference | null>(null);
	let search = $state('');
	const authored = $derived(JSON.stringify(surface.definition));
	const channels = $derived(
		media?.media.flatMap((item) =>
			item.channels.map((channel) => ({
				key: channelKey(channel.reference),
				reference: channel.reference,
				label: `${item.label} / ${channel.label}`
			}))
		) ?? []
	);
	const sourceKey = $derived(source ? channelKey(source) : '');
	const selectedChannel = $derived(channels.find((channel) => channel.key === sourceKey));
	const query = $derived(search.trim().toLowerCase());
	const matches = $derived(
		channels.filter((channel) => channel.label.toLowerCase().includes(query))
	);
	const visible = $derived(matches.slice(0, 100));
	$effect(() => {
		const definition: SurfaceDefinition = JSON.parse(authored);
		draft = definition;
		source = definition.source;
	});
	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!draft) return;
		await configure({ ...draft, source });
	}
</script>

{#if draft}
	<form onsubmit={submit}>
		<fieldset {disabled}>
			<legend>Surface Source · {surface.label}</legend>
			<label class="toggle"><input type="checkbox" bind:checked={draft.enabled} /> Enabled</label>
			<label
				>Find source<input
					type="search"
					bind:value={search}
					placeholder="Media or channel name" /></label>
			<label
				>Source channel<select
					value={sourceKey}
					onchange={(event) => {
						const key = event.currentTarget.value;
						if (key === '') source = null;
						else {
							const selected = channels.find((channel) => channel.key === key);
							if (selected) source = selected.reference;
						}
					}}>
					<option value="">No source</option>
					{#if source && !visible.some((channel) => channel.key === sourceKey)}
						<option value={sourceKey}
							>{selectedChannel?.label ?? `Missing source · ${source.channel}`}</option>
					{/if}
					{#each visible as channel (channel.key)}<option value={channel.key}
							>{channel.label}</option
						>{/each}
				</select></label>
			{#if matches.length > visible.length}<p class="hint">
					Showing {visible.length} of {matches.length} matching channels. Refine the search to find another
					source.
				</p>{/if}
			<div class="fields">
				<label
					>Opacity<input
						type="number"
						min="0"
						max="1"
						step="any"
						bind:value={draft.opacity}
						required /></label>
				<label
					>Calibration<select bind:value={draft.calibration}>
						<option value="Off">Off · use source</option><option value="Grid">Grid</option><option
							value="ColorBars">Color bars</option>
					</select></label>
			</div>
			<p class="hint">Calibration keeps the assigned source for when you switch it off.</p>
			<h4>Source crop</h4>
			<div class="fields">
				<label
					>Crop X<input
						type="number"
						min="0"
						max="1"
						step="any"
						bind:value={draft.crop.x}
						required /></label>
				<label
					>Crop Y<input
						type="number"
						min="0"
						max="1"
						step="any"
						bind:value={draft.crop.y}
						required /></label>
				<label
					>Crop width<input
						type="number"
						min="0"
						max="1"
						step="any"
						bind:value={draft.crop.width}
						required /></label>
				<label
					>Crop height<input
						type="number"
						min="0"
						max="1"
						step="any"
						bind:value={draft.crop.height}
						required /></label>
			</div>
			<h4>Destination corners</h4>
			<p class="hint">Screen fractions: 0 to 1 spans the screen; corners may extend outside it.</p>
			{#each ['Top left', 'Top right', 'Bottom right', 'Bottom left'] as label, index}
				<div class="fields">
					<label
						>{label} X<input
							type="number"
							step="any"
							bind:value={draft.quad.corners[index].x}
							required /></label>
					<label
						>{label} Y<input
							type="number"
							step="any"
							bind:value={draft.quad.corners[index].y}
							required /></label>
				</div>
			{/each}
			<button type="submit">Apply surface settings</button>
		</fieldset>
	</form>
{/if}
