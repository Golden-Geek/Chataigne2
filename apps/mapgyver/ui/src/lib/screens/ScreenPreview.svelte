<script lang="ts">
	import { onDestroy, type Snippet } from 'svelte';
	import { NativePreview, MAX_VIEW_OVERLAYS, type OverlayPrimitive } from 'golden_video_ui';
	import { appState, Viewport2D, pointBounds } from 'golden_ui';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { PreviewSource } from '../preview/generated/PreviewSource';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import SurfaceHandles from './SurfaceHandles.svelte';
	import type { SurfaceId } from './generated/SurfaceId';
	import type { SurfaceReference } from './generated/SurfaceReference';
	import type { Point } from './generated/Point';
	import { createSurfaceKeyboard, type KeyboardTarget } from './surface-keyboard';
	import type { SurfaceHandle } from './surface-gesture';
	let {
		screen,
		selected,
		disabled,
		select,
		start,
		pixelsActive,
		gestureActive,
		move,
		finish,
		stop,
		actions
	}: {
		screen: ScreenSnapshot;
		selected: SurfaceId | null;
		disabled: boolean;
		select(id: SurfaceId | null): void;
		start(reference: SurfaceReference, handle: SurfaceHandle): boolean;
		pixelsActive(): boolean;
		gestureActive: boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		actions?: Snippet;
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
	const documentIdentity = $derived(`${appState.session?.projectGeneration ?? 0}:${screen.id}`);
	let corner = $state<{ document: string; surface: SurfaceId; index: number } | null>(null);
	const keyboardTarget = $derived.by<KeyboardTarget | null>(() => {
		if (!corner || corner.document !== documentIdentity || corner.surface !== selected) return null;
		const surface = screen.surfaces.find((surface) => surface.reference.surface === selected);
		const origin = surface?.definition.quad.corners[corner.index];
		return surface && origin
			? { document: documentIdentity, reference: surface.reference, corner: corner.index, origin }
			: null;
	});
	const keyboard = createSurfaceKeyboard({
		start: (target) =>
			!disabled &&
			start(target.reference, {
				kind: 'QuadCornerPixels',
				corner: target.corner,
				origin: target.origin
			}),
		move: (pixels) => move(pixels),
		finish: () => {
			if (pixelsActive()) finish();
		},
		stop: () => {
			if (pixelsActive()) stop();
		}
	});
	const inputAvailable = $derived(
		appState.session?.status === 'connected' && !appState.session.loading.visible
	);
	$effect(() =>
		keyboard.observe(
			inputAvailable ? keyboardTarget : null,
			pixelsActive(),
			!disabled && !gestureActive
		)
	);
	onDestroy(() => keyboard.cancel());
	$effect(() => {
		if (corner && (corner.document !== documentIdentity || corner.surface !== selected))
			corner = null;
	});
	const identity = $derived<PreviewSource>({ kind: 'Screen', screen: screen.id });
	const source = $derived(JSON.stringify(identity));
	const status = $derived(owner.statuses.get(previewKey(identity)));
	const target = $derived(screen.definition.target);
	const extent = $derived(
		target?.kind === 'Rectangle'
			? target.bounds.extent
			: target?.kind === 'Spout'
				? target.extent
				: status?.extent
	);
	const selection = $derived(
		pointBounds(
			screen.surfaces.find((surface) => surface.reference.surface === selected)?.definition.quad
				.corners ?? []
		)
	);
</script>

<svelte:window onkeyup={(event) => keyboard.keyup(event.key)} onblur={() => keyboard.finish()} />

<section aria-label="Screen preview">
	<Viewport2D
		{actions}
		{nativeFrame}
		aspect={extent ? extent.width / extent.height : 16 / 9}
		identity={documentIdentity}
		onbackgroundclick={() => {
			if (disabled) return;
			corner = null;
			select(null);
		}}
		onkeydown={(event) => {
			if (event.key === 'Escape' && corner) {
				keyboard.cancel();
				corner = null;
				event.preventDefault();
			} else if (keyboard.keydown(event, inputAvailable ? keyboardTarget : null)) {
				event.preventDefault();
				event.stopPropagation();
			}
		}}
		onfocusout={() => keyboard.finish()}
		{selection}
		label="Screen view"
		native>
		<NativePreview
			nativeInput
			nativeBackdrop
			lifetime={documentIdentity}
			onpresentationchange={presentationChanged}
			overlayCapacity={MAX_VIEW_OVERLAYS}
			session={owner.session}
			{source}
			overlays={status?.error ? [] : overlays}
			label={`Preview of ${screen.label}`}>
			{#if status?.error}<div class="error" role="alert">{status.error}</div>{/if}
			{#key documentIdentity}
				<SurfaceHandles
					onoverlays={setOverlays}
					{screen}
					{selected}
					{disabled}
					{select}
					{start}
					{move}
					{finish}
					{stop}
					selectedCorner={corner &&
					corner.document === documentIdentity &&
					corner.surface === selected
						? corner.index
						: null}
					pickCorner={(surface, index) => {
						select(surface);
						corner = { document: documentIdentity, surface, index };
					}} />
			{/key}
		</NativePreview>
	</Viewport2D>
</section>

<style>
	section {
		height: 100%;
		min-height: 0;
	}
	.error {
		position: absolute;
		inset: 0;
		display: grid;
		place-content: center;
		padding: 1em;
		background: #000;
		overflow-wrap: anywhere;
	}
</style>
