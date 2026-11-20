<script lang="ts">
	import { VideoPreview } from 'golden_video_ui';
	import { appState } from 'golden_ui';
	import type { ChannelSnapshot } from './generated/ChannelSnapshot';
	import { usePreview } from '../preview/context.svelte';
	import { useViewSource } from '../preview/view-source.svelte';
	import PreviewMessage from '../preview/PreviewMessage.svelte';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	let {
		channel,
		fill = false,
		preview,
		label,
		extent: explicitExtent
	}: {
		channel: Pick<ChannelSnapshot, 'label' | 'reference'>;
		fill?: boolean;
		preview?: PreviewSource;
		label?: string;
		extent?: { width: number; height: number };
	} = $props();
	const owner = usePreview();
	const identity = $derived<PreviewSource>(
		preview ?? { kind: 'Channel', reference: channel.reference }
	);
	const view = useViewSource(() => identity);
	const status = $derived(view.status);
	const message = $derived(status?.error ?? status?.message);
	const extent = $derived(explicitExtent ?? view.extent);
</script>

<section aria-label="Channel preview" class:fill>
	<div
		class="surface"
		style:--preview-aspect={extent ? extent.width / extent.height : 16 / 9}
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<VideoPreview
			session={owner.session}
			source={view.wire}
			lifetime={appState.session?.projectGeneration ?? 0}
			onextent={view.onextent}
			label={`Preview of ${label ?? channel.label}`}>
			{#if message}<PreviewMessage {message} />{/if}
		</VideoPreview>
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
</style>
