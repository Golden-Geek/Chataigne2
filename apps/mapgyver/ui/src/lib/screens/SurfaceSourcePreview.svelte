<script lang="ts">
	import { NativePreview, type OverlayPrimitive } from 'golden_video_ui';
	import { appState, Viewport2D } from 'golden_ui';
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import type { Point } from './generated/Point';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import type { SurfaceHandle } from './surface-gesture';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import CropHandles from './CropHandles.svelte';
	let {
		surface,
		disabled,
		start,
		move,
		finish,
		stop
	}: {
		surface: SurfaceSnapshot;
		disabled: boolean;
		start(handle: SurfaceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	const owner = usePreview();
	let overlays = $state<OverlayPrimitive[]>([]);
	let nativeFrame = $state(false);
	function presentationChanged(painted: boolean) {
		nativeFrame = painted;
	}
	function setOverlays(next: OverlayPrimitive[]) {
		overlays = next;
	}
	const identity = $derived<PreviewSource | null>(
		surface.definition.source ? { kind: 'Channel', reference: surface.definition.source } : null
	);
	const source = $derived(JSON.stringify(identity));
	const status = $derived(identity ? owner.statuses.get(previewKey(identity)) : undefined);
	const extent = $derived(status?.extent);
</script>

{#snippet controls()}
	{#key surface.reference.surface + source}
		<CropHandles
			onoverlays={setOverlays}
			crop={surface.definition.crop}
			{disabled}
			{start}
			{move}
			{finish}
			{stop} />
	{/key}
{/snippet}

<section aria-label="Surface Source preview">
	<Viewport2D
		nativeFrame={identity !== null && nativeFrame}
		aspect={extent ? extent.width / extent.height : 16 / 9}
		identity={`${appState.session?.projectGeneration ?? 0}:${surface.reference.surface}`}
		selection={surface.definition.crop}
		label="Surface source view"
		native={identity !== null}>
		{#if identity}
			<NativePreview
				onpresentationchange={presentationChanged}
				overlayCapacity={16}
				session={owner.session}
				{source}
				overlays={status?.error || status?.message ? [] : overlays}
				label={`Full source for ${surface.label}`}>
				{#if status?.error || status?.message}<div class="message">
						{status.error ?? status.message}
					</div>{/if}
				{@render controls()}
			</NativePreview>
		{:else}
			<div class="message">No source assigned. Choose a channel in Surface settings.</div>
			{@render controls()}
		{/if}
	</Viewport2D>
</section>

<style>
	section {
		height: 100%;
		min-height: 0;
	}
	.message {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		padding: 1em;
		background: #000;
		overflow-wrap: anywhere;
		pointer-events: none;
	}
</style>
