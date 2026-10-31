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
	<div
		class="surface"
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		<NativePreview session={owner.session} {source} label={`Preview of ${screen.label}`}>
			{#if status?.error}<div class="error">{status.error}</div>
			{:else if status?.message}<div class="message">{status.message}</div>{/if}
			<SurfaceHandles {screen} {selected} {disabled} {select} {start} {move} {finish} {stop} />
		</NativePreview>
	</div>
	<p>
		Select a Surface and drag its corners. Invalid positions keep the last valid mapping. One Undo
		restores the whole drag. A stopped drag keeps its accepted position.
	</p>
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
