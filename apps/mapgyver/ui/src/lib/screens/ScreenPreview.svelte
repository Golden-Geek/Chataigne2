<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
	import { appState, Viewport2D, pointBounds } from 'golden_ui';
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

<section aria-label="Screen preview">
	<Viewport2D
		aspect={extent ? extent.width / extent.height : 16 / 9}
		identity={`${appState.session?.projectGeneration ?? 0}:${screen.id}`}
		{selection}
		label="Screen view"
		native>
		<NativePreview session={owner.session} {source} label={`Preview of ${screen.label}`}>
			{#if status?.error}<div class="error" role="alert">{status.error}</div>{/if}
			{#key screen.id}
				<SurfaceHandles {screen} {selected} {disabled} {select} {start} {move} {finish} {stop} />
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
