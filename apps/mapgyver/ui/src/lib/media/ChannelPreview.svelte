<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import { useMediaPreview } from './preview-context.svelte';
	import { channelKey } from './identity';
	let { channel }: { channel: ChannelSnapshot } = $props();
	const owner = useMediaPreview();
	const source = $derived(JSON.stringify(channel.reference));
	const status = $derived(owner.statuses.get(channelKey(channel.reference)));
	const extent = $derived(status?.resolution?.extent);
</script>

<section aria-label="Channel preview">
	<div
		class="surface"
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<NativePreview session={owner.session} {source} label={`Preview of ${channel.label}`}>
			{#if status && (status.error || status.resolution?.availability !== 'Ready')}
				<div class="message">
					{status.error ?? `Source ${status.resolution?.availability ?? 'unavailable'}`}
				</div>
			{/if}
		</NativePreview>
	</div>
</section>

<style>
	.surface {
		width: 100%;
		min-height: 7rem;
		background: transparent;
	}
	.message {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		padding: 1em;
		background: #202329;
		overflow-wrap: anywhere;
	}
</style>
