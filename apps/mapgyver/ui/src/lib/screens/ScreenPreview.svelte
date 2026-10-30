<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	let { screen }: { screen: ScreenSnapshot } = $props();
	const owner = usePreview();
	const identity = $derived<PreviewSource>({ kind: 'Screen', screen: screen.id });
	const source = $derived(JSON.stringify(identity));
	const status = $derived(owner.statuses.get(previewKey(identity)));
	const extent = $derived(status?.extent);
</script>

<section aria-label="Screen preview">
	<div
		class="surface"
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<NativePreview session={owner.session} {source} label={`Preview of ${screen.label}`}>
			{#if status?.error}<div class="error">{status.error}</div>
			{:else if status?.message}<div class="message">{status.message}</div>{/if}
		</NativePreview>
	</div>
	<p>Preview stays available while the physical output is disabled.</p>
</section>

<style>
	.surface {
		width: 100%;
		min-height: 7rem;
		background: transparent;
	}
	.error {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		padding: 1em;
		background: #202329;
		overflow-wrap: anywhere;
	}
	.message {
		position: absolute;
		inset: auto 0 0;
		padding: 0.5em;
		background: #202329d9;
		overflow-wrap: anywhere;
	}
	p {
		font-size: 0.85em;
		opacity: 0.65;
		margin: 0.4em 0 0;
	}
</style>
