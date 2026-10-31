<script lang="ts">
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import type { MediaManagerSnapshot } from '../media/generated/MediaManagerSnapshot';
	import { channelKey } from '../media/identity';
	let {
		catalog,
		selected = null,
		name = 'source',
		emptyLabel = 'No source',
		required = false
	}: {
		catalog: MediaManagerSnapshot;
		selected?: ChannelReference | null;
		name?: string;
		emptyLabel?: string;
		required?: boolean;
	} = $props();
	const key = $derived(selected ? channelKey(selected) : '');
	let value = $state('');
	let search = $state('');
	$effect(() => {
		value = key;
	});
	const selectedChoice = $derived.by(() => {
		for (const media of catalog.media) {
			const channel = media.channels.find((channel) => channelKey(channel.reference) === value);
			if (channel) return { key: value, label: `${media.label} / ${channel.label}` };
		}
		return null;
	});
	const choices = $derived.by(() => {
		const result: { key: string; label: string }[] = [];
		const query = search.toLocaleLowerCase();
		for (const media of catalog.media) {
			for (const channel of media.channels) {
				const label = `${media.label} / ${channel.label}`;
				if (label.toLocaleLowerCase().includes(query)) {
					result.push({ key: channelKey(channel.reference), label });
					if (result.length === 100) return result;
				}
			}
		}
		return result;
	});
</script>

<input
	type="search"
	bind:value={search}
	placeholder="Find media or channel…"
	aria-label="Find source channel" />
<select {name} {required} bind:value aria-label="Source channel">
	<option value="" disabled={required}>{emptyLabel}</option>
	{#if value && !choices.some((choice) => choice.key === value)}
		<option {value}
			>{selectedChoice?.label ?? `Missing channel (${selected?.channel ?? value})`}</option>
	{/if}
	{#each choices as choice (choice.key)}
		<option value={choice.key}>{choice.label}</option>
	{/each}
</select>
{#if choices.length === 100}<small>Showing up to 100 matches. Narrow the search to find more.</small
	>{/if}
