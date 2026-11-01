<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import SurfaceHandles from './SurfaceHandles.svelte';
	import type { SurfaceId } from './generated/SurfaceId';
	import type { SurfaceReference } from './generated/SurfaceReference';
	import type { Point } from './generated/Point';
	let {
		screen,
		selected,
		disabled,
		select,
		start,
		move,
		finish,
		stop
	}: {
		screen: ScreenSnapshot;
		selected: SurfaceId | null;
		disabled: boolean;
		select(id: SurfaceId): void;
		start(reference: SurfaceReference, corner: number): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	const owner = usePreview();
	const identity = $derived<PreviewSource>({ kind: 'Screen', screen: screen.id });
	const source = $derived(JSON.stringify(identity));
	const status = $derived(owner.statuses.get(previewKey(identity)));
	const extent = $derived(status?.extent);
</script>

<section aria-label="Screen preview">
	<div class="surface" style:--preview-aspect={extent ? extent.width / extent.height : 16 / 9}>
		<NativePreview session={owner.session} {source} label={`Preview of ${screen.label}`}>
			{#if status?.error}<div class="error">{status.error}</div>
			{:else if status?.message}<div class="message">{status.message}</div>{/if}
			<SurfaceHandles {screen} {selected} {disabled} {select} {start} {move} {finish} {stop} />
		</NativePreview>
	</div>
</section>

<style>
	section {
		height: 100%;
		display: grid;
		place-items: center;
		min-height: 0;
	}
	.surface {
		width: min(100%, calc(100cqh * var(--preview-aspect)));
		aspect-ratio: var(--preview-aspect);
		max-height: 100%;
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
</style>
