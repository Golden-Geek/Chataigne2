<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
	import { capturePreviewPointer, type OverlayPrimitive } from 'golden_video_ui';
	import { overlayRem } from './overlays';
	import { cropOverlays } from './crop-overlays';
	import type { UnitRect } from './generated/UnitRect';
	import type { Point } from './generated/Point';
	import type { SurfaceHandle } from './surface-gesture';
	import type { SurfacePresentation } from './surface-gesture';
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import { cropCorners } from './gesture-presentation';
	import { pointBounds } from 'golden_ui';
	let {
		crop,
		selected,
		select,
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
		selected: boolean;
		select(handle: 'origin' | 'size'): boolean;
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
	let drag: {
		id: number;
		offset: Point;
		x: number;
		y: number;
		threshold: number;
		moved: boolean;
	} | null = null;
	let ghost = $state<Point | null>(null);
	const corners = $derived(cropCorners(surface, presentation));
	const bounds = $derived(pointBounds(corners)!);
	$effect(() => {
		onoverlays(cropOverlays(corners, selected, ghost, overlayRem()));
	});
	$effect(() => {
		if (!selected) cancel();
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
		if (drag || event.button !== 0 || event.defaultPrevented) return;
		const editable = !disabled;
		event.preventDefault();
		event.stopPropagation();
		if (!select(handle.kind === 'CropOrigin' ? 'origin' : 'size')) return;
		const origin = point(event);
		if (!editable || !origin || !start(handle)) return;
		drag = {
			id: event.pointerId,
			x: event.clientX,
			y: event.clientY,
			threshold: overlayRem() * 0.125,
			moved: false,
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
		if (!drag.moved) {
			if (Math.hypot(event.clientX - drag.x, event.clientY - drag.y) < drag.threshold) return;
			drag.moved = true;
		}
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
		const previous = drag;
		drag = null;
		ghost = null;
		if (canvas.hasPointerCapture(previous.id)) canvas.releasePointerCapture(previous.id);
		stop();
	}
	function release(event: PointerEvent) {
		if (event.pointerId !== drag?.id) return;
		update(event);
		if (!drag) return;
		const previous = drag;
		drag = null;
		ghost = null;
		if (previous.moved) finish();
		else stop();
		if (canvas.hasPointerCapture(previous.id)) canvas.releasePointerCapture(previous.id);
	}
	export function keydown(event: KeyboardEvent) {
		if (event.key === 'Escape' && drag) {
			event.preventDefault();
			event.stopPropagation();
			cancel();
		}
	}
	onDestroy(cancel);
</script>

<svelte:window onkeydown={keydown} onblur={cancel} />
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
		role="button"
		tabindex="0"
		aria-label="Select source crop"
		aria-pressed={selected}
		class:selected
		x={bounds.x}
		y={bounds.y}
		width={bounds.width}
		height={bounds.height}
		onkeydown={(event) => {
			if (event.key === 'Enter' || event.key === ' ') {
				event.preventDefault();
				event.stopPropagation();
				select('origin');
			}
		}}
		onclick={(event) => {
			if (event.detail === 0) select('origin');
		}}
		onpointerdown={(event) => down(event, { kind: 'CropOrigin' })} />
</svg>

{#if selected}{#each corners as corner, index}
		<ViewportHandle
			data-native-hit-id={`crop:${index}`}
			data-native-overlay-control
			x={corner.x}
			y={corner.y}
			label={`Crop corner ${index + 1}`}
			aria-disabled={disabled}
			onclick={(event) => {
				if (event.detail === 0) select('size');
			}}
			onpointerdown={(event) => down(event, { kind: 'CropCorner', corner: index })} />
	{/each}{/if}
{#if selected && ghost}<ViewportHandle
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
		fill: transparent;
		stroke: #a2b0c5;
		stroke-width: 0.0625rem;
		vector-effect: non-scaling-stroke;
		cursor: move;
		pointer-events: all;
	}
	.selected {
		fill: #78aafa18;
		stroke: #70aaff;
		stroke-width: 0.125rem;
	}
</style>
