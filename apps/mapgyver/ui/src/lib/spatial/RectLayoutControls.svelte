<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		ViewportHandle,
		ViewportTarget,
		ViewportGuides,
		viewportContour,
		type AlignmentGuide
	} from 'golden_ui';
	import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
	import type { Point } from '../media/generated/Point';
	import type { LayoutRect } from './rect-presentation';
	import { displayRect, projectRect } from './rect-presentation';
	import { createGeometryAlignment } from './alignment';
	import type { LayerPresentation } from './layer-gesture';
	import type { CellPresentation } from './cell-gesture';
	import { createViewportPointer } from 'golden_ui';
	import { SPATIAL_RESIZE_HANDLES } from './handles';
	let {
		objects,
		snapping = false,
		presentation = null,
		kind,
		identity,
		canvas,
		selected,
		disabled,
		selectionAvailable,
		select,
		active,
		start,
		move,
		finish,
		stop,
		enter
	}: {
		objects: LayoutRect[];
		snapping?: boolean;
		presentation?: LayerPresentation | CellPresentation | null;
		kind: 'Layer' | 'Cell';
		identity?: string;
		canvas: { width: number; height: number };
		selected: string | null;
		disabled: boolean;
		selectionAvailable: boolean;
		select(uuid: string): void;
		active: boolean;
		start(id: string, mode: RectGeometryMode, point: Point): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		enter?(id: string): void;
	} = $props();
	let svg: SVGSVGElement;
	let ghost = $state<Point | null>(null);
	let guides = $state<AlignmentGuide[]>([]);
	const alignment = createGeometryAlignment(
		() => svg,
		() => snapping,
		(next) => {
			guides = next;
		}
	);
	const bounds = (object: LayoutRect) =>
		object.size
			? {
					x: object.position[0],
					y: object.position[1],
					width: object.size.width,
					height: object.size.height
				}
			: null;
	const pointer = createViewportPointer({
		canvas: () => svg,
		extent: () => canvas,
		ghost: (at) => {
			ghost = at;
		},
		move: (at) => move(alignment.move(at)),
		finish: () => {
			alignment.clear();
			finish();
		},
		stop: () => {
			alignment.clear();
			stop();
		}
	});
	const displayed = $derived(objects.map((object) => displayRect(object, presentation)));
	const item = $derived(displayed.find((object) => object.id === selected));
	// Cells are alternative Column states. Keep the selected Cell's controls above
	// overlapping alternatives; Layers retain their backend-defined compositing order.
	const paintOrder = $derived(
		kind === 'Cell' && item
			? [...displayed.filter((object) => object.id !== item.id), item]
			: displayed
	);
	function down(event: PointerEvent, object: LayoutRect, mode: RectGeometryMode) {
		pointer.start(event, {
			selectable: selectionAvailable && !active,
			editable: !disabled,
			select: () => select(object.id),
			begin: (at) => {
				if (!start(object.id, mode, at)) return false;
				const origin = {
					position: [...object.position] as [number, number],
					size: object.size ? { ...object.size } : null
				};
				alignment.begin(
					objects.map((item) => ({ id: item.id, bounds: bounds(item) })),
					object.id,
					canvas,
					(point) => bounds(projectRect(object, origin, mode, at, point)),
					{ centers: mode === 'Translate' }
				);
				return true;
			}
		});
	}
	$effect(() => {
		if (!active) pointer.cancel();
	});
	onDestroy(pointer.cancel);
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && pointer.active) {
			event.preventDefault();
			pointer.cancel();
		}
	}} />

<svg
	bind:this={svg}
	viewBox={`0 0 ${canvas.width} ${canvas.height}`}
	preserveAspectRatio="none"
	role="group"
	aria-label={`Matrix ${kind} layout controls`}
	data-geometry-presentation={presentation ? 'pending' : 'committed'}
	onpointermove={pointer.move}
	onpointerup={(event) => pointer.release(event)}
	onpointercancel={(event) => pointer.release(event, true)}
	onlostpointercapture={pointer.cancel}>
	{#each paintOrder as object (object.id)}
		{#if object.size}
			<rect
				use:viewportContour={{
					selected: selected === object.id,
					enabled: object.enabled,
					identity
				}}
				data-layer-id={kind === 'Layer' ? object.id : undefined}
				data-layout-cell={kind === 'Cell' ? object.id : undefined}
				x={object.position[0]}
				y={object.position[1]}
				width={object.size.width}
				height={object.size.height}
				class:selected={selected === object.id}
				class:inactive={!object.enabled}
				role="button"
				tabindex={selectionAvailable ? 0 : -1}
				aria-label={`Select ${object.label}`}
				aria-disabled={!selectionAvailable || active}
				onpointerdown={(event) => down(event, object, 'Translate')}
				ondblclick={(event) => {
					event.stopPropagation();
					enter?.(object.id);
				}}
				onclick={(event) => {
					event.stopPropagation();
					if (selectionAvailable && !active && event.detail === 0) select(object.id);
				}}
				onkeydown={(event) => {
					if (selectionAvailable && !active && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(object.id);
					}
				}} />
		{/if}
	{/each}
</svg>
<ViewportGuides {guides} width={canvas.width} height={canvas.height} />

{#each displayed as object (object.id)}
	{#if !object.size && object.id !== selected}<ViewportTarget
			{identity}
			x={object.position[0] / canvas.width}
			y={object.position[1] / canvas.height}
			data-layout-cell={kind === 'Cell' ? object.id : undefined}
			label={`Select ${object.label}`}
			enabled={object.enabled}
			aria-disabled={!selectionAvailable || active}
			onclick={(event) => {
				if (selectionAvailable && !active && event.detail === 0) select(object.id);
			}}
			onpointerdown={(event) => down(event, object, 'Translate')} />{/if}
{/each}

{#if item && !item.size}
	<ViewportHandle
		x={item.position[0] / canvas.width}
		y={item.position[1] / canvas.height}
		data-layout-cell={kind === 'Cell' ? item.id : undefined}
		label={`Move ${item.label}`}
		aria-disabled={disabled && !active}
		onpointerdown={(event) => down(event, item!, 'Translate')} />
{/if}

{#if item?.size}
	{#each SPATIAL_RESIZE_HANDLES as handle (handle.name)}
		<ViewportHandle
			x={(item.position[0] + handle.x * item.size.width) / canvas.width}
			y={(item.position[1] + handle.y * item.size.height) / canvas.height}
			label={`Resize ${item.label} ${handle.name}`}
			tabindex={disabled ? -1 : 0}
			data-matrix-layer-handle={kind === 'Layer' ? 'Resize' : undefined}
			data-matrix-layer-resize={kind === 'Layer' ? handle.name : undefined}
			data-matrix-cell-resize={kind === 'Cell' ? handle.name : undefined}
			style={`border-radius: 0.1rem; cursor: ${handle.cursor}`}
			aria-disabled={disabled && !active}
			onpointerdown={(event) => {
				if (item) down(event, item, { Resize: { handle: handle.name } });
			}} />
	{/each}
{/if}
{#if ghost}<ViewportHandle
		x={ghost.x / canvas.width}
		y={ghost.y / canvas.height}
		label="Transform pointer"
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
		pointer-events: all;
		cursor: move;
	}
</style>
