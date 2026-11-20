<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle, ViewportGuides, viewportContour, type AlignmentGuide } from 'golden_ui';
	import { createGeometryAlignment } from '../spatial/alignment';
	import type { RelatedSurface } from './generated/RelatedSurface';
	import { capturePreviewPointer, type OverlayPrimitive } from 'golden_video_ui';
	import { overlayRem, quadOverlay } from '../preview/overlays';
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
		snapping = false,
		related = [],
		selected,
		selectionAvailable,
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
		snapping?: boolean;
		related?: RelatedSurface[];
		selected: boolean;
		selectionAvailable: boolean;
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
	let guides = $state<AlignmentGuide[]>([]);
	const alignment = createGeometryAlignment(
		() => canvas,
		() => snapping,
		(next) => {
			guides = next;
		}
	);
	let hovered = $state(false);
	let focused = $state(false);
	let focusedCorner = $state<number | null>(null);
	const corners = $derived(cropCorners(surface, presentation));
	const bounds = $derived(pointBounds(corners)!);
	$effect(() => {
		onoverlays([
			...cropOverlays(corners, selected, ghost, overlayRem(), hovered || focused, focusedCorner),
			...guides.map((guide) => ({
				...quadOverlay(
					guide.axis === 'x'
						? [
								{ x: guide.value, y: 0 },
								{ x: guide.value, y: 1 },
								{ x: guide.value, y: 1 },
								{ x: guide.value, y: 0 }
							]
						: [
								{ x: 0, y: guide.value },
								{ x: 1, y: guide.value },
								{ x: 1, y: guide.value },
								{ x: 0, y: guide.value }
							],
					overlayRem(),
					{ color: '70aaffcc' }
				),
				dash: [overlayRem() * 0.25, overlayRem() * 0.2] as [number, number]
			}))
		]);
	});
	$effect(() => {
		if (!selected || !selectionAvailable) cancel();
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
		if (!selectionAvailable || drag || event.button !== 0 || event.defaultPrevented) return;
		const editable = !disabled;
		event.preventDefault();
		event.stopPropagation();
		if (!select(handle.kind === 'CropOrigin' ? 'origin' : 'size')) return;
		const origin = point(event);
		if (!editable || !origin || !start(handle)) return;
		const initial = { ...surface, definition: { ...surface.definition, crop: { ...crop } } };
		alignment.begin(
			related.map((entry) => ({
				id: entry.surface.reference.surface,
				bounds: pointBounds(cropCorners(entry.surface, null))
			})),
			surface.reference.surface,
			{ width: 1, height: 1 },
			(sample) =>
				pointBounds(cropCorners(initial, { reference: initial.reference, handle, sample })),
			{ centers: handle.kind === 'CropOrigin' }
		);
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
		move(alignment.move({ x: next.x - drag.offset.x, y: next.y - drag.offset.y }));
	}
	function cancel() {
		if (!drag) return;
		const previous = drag;
		drag = null;
		ghost = null;
		alignment.clear();
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
		alignment.clear();
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
		use:viewportContour={{
			selected,
			onhover: (value) => {
				hovered = value;
			},
			onfocus: (value) => {
				focused = value;
			}
		}}
		data-native-hit-id="crop"
		role="button"
		tabindex={selectionAvailable ? 0 : -1}
		aria-disabled={!selectionAvailable}
		aria-label="Select source crop"
		aria-pressed={selected}
		class:selected
		x={bounds.x}
		y={bounds.y}
		width={bounds.width}
		height={bounds.height}
		onkeydown={(event) => {
			if (selectionAvailable && (event.key === 'Enter' || event.key === ' ')) {
				event.preventDefault();
				event.stopPropagation();
				select('origin');
			}
		}}
		onclick={(event) => {
			if (selectionAvailable && event.detail === 0) select('origin');
		}}
		onpointerdown={(event) => down(event, { kind: 'CropOrigin' })} />
</svg>
<ViewportGuides {guides} width={1} height={1} native />

{#if selected}{#each corners as corner, index}
		<ViewportHandle
			data-native-hit-id={`crop:${index}`}
			data-native-overlay-control
			x={corner.x}
			y={corner.y}
			label={`Crop corner ${index + 1}`}
			tabindex={selectionAvailable ? 0 : -1}
			onfocus={() => {
				focusedCorner = index;
			}}
			onblur={() => {
				focusedCorner = null;
			}}
			disabled={!selectionAvailable}
			aria-disabled={disabled}
			onclick={(event) => {
				if (selectionAvailable && event.detail === 0) select('size');
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
		cursor: move;
		pointer-events: all;
	}
</style>
