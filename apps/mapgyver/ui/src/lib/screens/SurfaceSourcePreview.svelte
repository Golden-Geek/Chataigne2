<script lang="ts">
	import { VideoPreview, type OverlayPrimitive } from 'golden_video_ui';
	import { appState, Viewport2D, pointBounds } from 'golden_ui';
	import { hasDesktopHost } from 'golden_ui/host/desktop';
	import type { Snippet } from 'svelte';
	import type { RelatedSurface } from './generated/RelatedSurface';
	import RelatedContours from './RelatedContours.svelte';
	import { cropCorners } from './gesture-presentation';
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import type { Point } from './generated/Point';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import type { SurfaceHandle, SurfacePresentation } from './surface-gesture';
	import { usePreview } from '../preview/context.svelte';
	import { useViewSource } from '../preview/view-source.svelte';
	import PreviewMessage from '../preview/PreviewMessage.svelte';
	import CropHandles from './CropHandles.svelte';
	import AlignmentToggle from '../spatial/AlignmentToggle.svelte';
	let {
		surface,
		preview,
		inputAvailable,
		selected,
		select,
		onselectionclear,
		presentation = null,
		disabled,
		start,
		move,
		finish,
		stop,
		actions,
		related = [],
		selectRelated = () => {}
	}: {
		surface: SurfaceSnapshot;
		preview: PreviewSource | null;
		inputAvailable: boolean;
		selected: boolean;
		select(handle: 'origin' | 'size'): boolean;
		onselectionclear(): void;
		presentation?: SurfacePresentation | null;
		disabled: boolean;
		start(handle: SurfaceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		actions?: Snippet;
		related?: RelatedSurface[];
		selectRelated?(uuid: string): void;
	} = $props();
	const owner = usePreview();
	let snapping = $state(false);
	let cropControls = $state<{ keydown(event: KeyboardEvent): void }>();
	let overlays = $state<OverlayPrimitive[]>([]);
	let relatedOverlays = $state<OverlayPrimitive[]>([]);
	let nativeFrame = $state(false);
	function presentationChanged(painted: boolean) {
		nativeFrame = painted;
	}
	function setOverlays(next: OverlayPrimitive[]) {
		overlays = next;
	}
	const identity = $derived(preview);
	const view = useViewSource(() => identity);
	const project = $derived(appState.session?.projectGeneration ?? 0);
	const source = $derived(view.wire);
	const status = $derived(view.status);
	const message = $derived(status?.error ?? status?.message);
	const extent = $derived(identity?.kind === 'Calibration' ? identity.extent : view.extent);
</script>

{#snippet controls()}
	<RelatedContours
		surfaces={related}
		selected={appState.session?.selectedNodeUuid ?? null}
		available={inputAvailable && !presentation}
		select={selectRelated}
		onoverlays={(next) => {
			relatedOverlays = next;
		}} />
	{#key `${project}:${surface.reference.surface}:${source}`}
		<CropHandles
			{snapping}
			{related}
			bind:this={cropControls}
			{surface}
			{selected}
			selectionAvailable={inputAvailable}
			{select}
			{presentation}
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
	{#snippet toolbar()}
		<AlignmentToggle bind:enabled={snapping} disabled={!!presentation} />
		{@render actions?.()}
	{/snippet}
	<Viewport2D
		actions={toolbar}
		nativeFrame={identity !== null && nativeFrame}
		aspect={extent ? extent.width / extent.height : 16 / 9}
		identity={`${project}:${surface.reference.surface}`}
		selection={selected ? pointBounds(cropCorners(surface, presentation)) : null}
		onselectionclear={() => {
			if (inputAvailable) onselectionclear();
		}}
		onkeydown={(event) => cropControls?.keydown(event)}
		label="Surface source view"
		native={identity !== null && hasDesktopHost()}>
		{#if identity}
			<VideoPreview
				nativeInput={inputAvailable}
				nativeBackdrop
				lifetime={project}
				onpresentationchange={presentationChanged}
				overlayCapacity={80}
				session={owner.session}
				{source}
				onextent={view.onextent}
				browserFill
				overlays={message ? [] : [...relatedOverlays, ...overlays]}
				label={`Full source for ${surface.label}`}>
				{#if message}<PreviewMessage {message} />{/if}
				{@render controls()}
			</VideoPreview>
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
