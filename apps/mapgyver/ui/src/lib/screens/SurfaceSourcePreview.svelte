<script lang="ts">
	import { NativePreview } from 'golden_video_ui';
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
	const identity = $derived<PreviewSource | null>(
		surface.definition.source ? { kind: 'Channel', reference: surface.definition.source } : null
	);
	const source = $derived(JSON.stringify(identity));
	const status = $derived(identity ? owner.statuses.get(previewKey(identity)) : undefined);
	const extent = $derived(status?.extent);
</script>

{#snippet controls()}
	<CropHandles crop={surface.definition.crop} {disabled} {start} {move} {finish} {stop} />
{/snippet}

<section aria-label="Surface Source preview">
	<h3>Surface Source · {surface.label}</h3>
	<div
		class="surface"
		style:aspect-ratio={extent ? `${extent.width} / ${extent.height}` : '16 / 9'}>
		{#if identity}
			{#key source}
				<NativePreview session={owner.session} {source} label={`Full source for ${surface.label}`}>
					{#if status?.error || status?.message}<div class="message">
							{status.error ?? status.message}
						</div>{/if}
					{@render controls()}
				</NativePreview>
			{/key}
		{:else}
			<div class="message">No source assigned. Choose a channel in Surface settings.</div>
			{@render controls()}
		{/if}
	</div>
	<p>
		The full source stays visible. Drag crop corners to resize, or drag inside to move the crop.
		Invalid positions keep the last valid crop. One Undo restores the whole drag.
	</p>
	{#if surface.definition.calibration !== 'Off'}
		<p>Calibration keeps this crop for when you switch it off.</p>
	{/if}
</section>

<style>
	.surface {
		position: relative;
		width: 100%;
		min-height: 7rem;
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
		pointer-events: none;
	}
	p {
		font-size: 0.85em;
		opacity: 0.85;
	}
</style>
