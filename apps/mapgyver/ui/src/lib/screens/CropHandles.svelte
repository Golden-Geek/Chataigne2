<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
	import { capturePreviewPointer, type OverlayPrimitive } from 'golden_video_ui';
	import { handleOverlays, imageFrameOverlay, overlayRem, quadOverlay } from './overlays';
	import type { UnitRect } from './generated/UnitRect';
	import type { Point } from './generated/Point';
	import type { SurfaceHandle } from './surface-gesture';
	import type { SurfacePresentation } from './surface-gesture';
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import { cropCorners } from './gesture-presentation';
	import { pointBounds } from 'golden_ui';
	let {
		crop,
		surface,
		presentation = null,
		disabled,
		start,
		move,
		finish,
		stop,
		onoverlays = () => {}
	}: {
		crop: UnitRect;
		surface: SurfaceSnapshot;
		presentation?: SurfacePresentation | null;
		disabled: boolean;
		start(handle: SurfaceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		onoverlays?(overlays: OverlayPrimitive[]): void;
	} = $props();
	let canvas: SVGSVGElement;
	let drag: { id: number; offset: Point } | null = null;
	let ghost = $state<Point | null>(null);
	const corners = $derived(cropCorners(surface, presentation));
	const bounds = $derived(pointBounds(corners)!);
	$effect(() => {
		const rem = overlayRem();
		onoverlays([
			imageFrameOverlay(rem),
			quadOverlay(corners, rem, { selected: true, hit: 'crop' }),
			...corners.flatMap((point, index) =>
				handleOverlays(point, rem, false, false, `crop:${index}`)
			),
			...(ghost ? handleOverlays(ghost, rem, false, true) : [])
		]);
	});
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		return rect.width > 0 && rect.height > 0
			? {
					x: Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)),
					y: Math.max(0, Math.min(1, (event.clientY - rect.top) / rect.height))
				}
			: null;
	}
	function down(event: PointerEvent, handle: SurfaceHandle) {
		const origin = point(event);
		if (disabled || drag || event.button !== 0 || !origin || !start(handle)) return;
		event.preventDefault();
		event.stopPropagation();
		drag = {
			id: event.pointerId,
			offset:
				handle.kind === 'CropOrigin'
					? { x: origin.x - crop.x, y: origin.y - crop.y }
					: { x: 0, y: 0 }
		};
		ghost = origin;
		capturePreviewPointer(canvas, event);
	}
	function update(event: PointerEvent) {
		if (event.pointerId !== drag?.id) return;
		const next = point(event);
		if (!next) {
			cancel();
			return;
		}
		ghost = next;
		move({ x: next.x - drag.offset.x, y: next.y - drag.offset.y });
	}
	function cancel() {
		if (!drag) return;
		const previous = drag.id;
		drag = null;
		ghost = null;
		if (canvas.hasPointerCapture(previous)) canvas.releasePointerCapture(previous);
		stop();
	}
	function release(event: PointerEvent) {
		if (event.pointerId !== drag?.id) return;
		update(event);
		if (!drag) return;
		const previous = drag.id;
		drag = null;
		ghost = null;
		finish();
		if (canvas.hasPointerCapture(previous)) canvas.releasePointerCapture(previous);
	}
	onDestroy(cancel);
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && drag) {
			event.preventDefault();
			cancel();
		}
	}} />
<svg
	data-native-overlay-control
	bind:this={canvas}
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	role="group"
	aria-label="Source crop controls"
	data-geometry-presentation={presentation ? 'pending' : 'committed'}
	onpointermove={update}
	onpointerup={release}
	onpointercancel={(event) => {
		if (event.pointerId === drag?.id) cancel();
	}}
	onlostpointercapture={(event) => {
		if (event.pointerId === drag?.id) cancel();
	}}>
	<rect
		data-native-hit-id="crop"
		x={bounds.x}
		y={bounds.y}
		width={bounds.width}
		height={bounds.height}
		aria-hidden="true"
		onpointerdown={(event) => down(event, { kind: 'CropOrigin' })} />
</svg>

{#each corners as corner, index}
	<ViewportHandle
		data-native-hit-id={`crop:${index}`}
		data-native-overlay-control
		x={corner.x}
		y={corner.y}
		label={`Crop corner ${index + 1}`}
		{disabled}
		onpointerdown={(event) => down(event, { kind: 'CropCorner', corner: index })} />
{/each}
{#if ghost}<ViewportHandle
		data-native-overlay-control
		x={ghost.x}
		y={ghost.y}
		label="Crop pointer"
		ghost />{/if}

<style>
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		touch-action: none;
		pointer-events: none;
	}
	rect {
		fill: #78aafa12;
		stroke: #70aaff;
		stroke-width: 0.1rem;
		vector-effect: non-scaling-stroke;
		cursor: move;
		pointer-events: all;
	}
</style>
