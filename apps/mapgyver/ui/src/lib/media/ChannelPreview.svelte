<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	let { channel, fill = false }: { channel: ChannelSnapshot; fill?: boolean } = $props();
	const owner = usePreview();
	const identity = $derived<PreviewSource>({ kind: 'Channel', reference: channel.reference });
	const source = $derived(JSON.stringify(identity));
	const status = $derived(owner.statuses.get(previewKey(identity)));
	const extent = $derived(status?.extent);
</script>

<section aria-label="Channel preview" class:fill>
	<div
		class="surface"
		style:--preview-aspect={extent ? extent.width / extent.height : 16 / 9}
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<NativePreview session={owner.session} {source} label={`Preview of ${channel.label}`}>
			{#if status && (status.error || status.message)}
				<div class="message">
					{status.error ?? status.message}
				</div>
			{/if}
		</NativePreview>
	</div>
</section>

<style>
	.fill,
	.fill .surface {
		height: 100%;
		min-height: 0;
	}
	.fill .surface {
		width: 100%;
	}
	.surface {
		width: min(100%, calc(10rem * var(--preview-aspect)));
		margin: auto;
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
