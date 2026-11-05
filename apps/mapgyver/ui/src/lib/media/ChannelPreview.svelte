<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	let {
		channel,
		fill = false,
		preview,
		label,
		extent: explicitExtent
	}: {
		channel: ChannelSnapshot;
		fill?: boolean;
		preview?: PreviewSource;
		label?: string;
		extent?: { width: number; height: number };
	} = $props();
	const owner = usePreview();
	const identity = $derived<PreviewSource>(
		preview ?? { kind: 'Channel', reference: channel.reference }
	);
	const source = $derived(JSON.stringify(identity));
	const status = $derived(owner.statuses.get(previewKey(identity)));
	const extent = $derived(explicitExtent ?? status?.extent);
</script>

<section aria-label="Channel preview" class:fill>
	<div
		class="surface"
		style:--preview-aspect={extent ? extent.width / extent.height : 16 / 9}
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<NativePreview session={owner.session} {source} label={`Preview of ${label ?? channel.label}`}>
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
	.fill {
		container-type: size;
		display: grid;
		place-items: center;
	}
	.fill .surface {
		width: min(100cqw, calc(100cqh * var(--preview-aspect)));
		height: min(100cqh, calc(100cqw / var(--preview-aspect)));
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
		background: #000;
		overflow-wrap: anywhere;
	}
</style>
