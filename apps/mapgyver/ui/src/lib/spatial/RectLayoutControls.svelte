<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
	import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
	import type { Point } from '../media/generated/Point';
	import type { LayoutRect } from './rect-presentation';
	import { createSpatialPointer } from './pointer';
	import { SPATIAL_RESIZE_HANDLES } from './handles';
	let {
		objects,
		kind,
		canvas,
		selected,
		disabled,
		select,
		active,
		start,
		move,
		finish,
		stop,
		enter
	}: {
		objects: LayoutRect[];
		kind: 'Layer' | 'Cell';
		canvas: { width: number; height: number };
		selected: string | null;
		disabled: boolean;
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
	const pointer = createSpatialPointer({
		canvas: () => svg,
		extent: () => canvas,
		ghost: (at) => {
			ghost = at;
		},
		move: (at) => move(at),
		finish: () => finish(),
		stop: () => stop()
	});
	const item = $derived(objects.find((object) => object.id === selected));
	function down(event: PointerEvent, object: LayoutRect, mode: RectGeometryMode) {
		if (disabled) return;
		pointer.start(event, (at) => {
			if (!start(object.id, mode, at)) return false;
			select(object.id);
			return true;
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
	onpointermove={pointer.move}
	onpointerup={(event) => pointer.release(event)}
	onpointercancel={(event) => pointer.release(event, true)}
	onlostpointercapture={pointer.cancel}>
	{#each objects as object (object.id)}
		{#if object.size}
			<rect
				data-layer-id={kind === 'Layer' ? object.id : undefined}
				data-layout-cell={kind === 'Cell' ? object.id : undefined}
				x={object.position[0]}
				y={object.position[1]}
				width={object.size.width}
				height={object.size.height}
				class:selected={selected === object.id}
				class:inactive={!object.enabled}
				role="button"
				tabindex={disabled ? -1 : 0}
				aria-label={`Select ${object.label}`}
				aria-disabled={disabled}
				onpointerdown={(event) => down(event, object, 'Translate')}
				ondblclick={(event) => {
					event.stopPropagation();
					enter?.(object.id);
				}}
				onclick={(event) => {
					event.stopPropagation();
					if (!disabled) select(object.id);
				}}
				onkeydown={(event) => {
					if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(object.id);
					}
				}} />
		{/if}
	{/each}
</svg>

{#each objects as object (object.id)}
	{#if !object.size}<ViewportHandle
			x={object.position[0] / canvas.width}
			y={object.position[1] / canvas.height}
			data-layout-cell={kind === 'Cell' ? object.id : undefined}
			label={`Move ${object.label}`}
			{disabled}
			onclick={() => select(object.id)}
			onpointerdown={(event) => down(event, object, 'Translate')} />{/if}
{/each}

{#if item?.size}
	{#each SPATIAL_RESIZE_HANDLES as handle (handle.name)}
		<ViewportHandle
			x={(item.position[0] + handle.x * item.size.width) / canvas.width}
			y={(item.position[1] + handle.y * item.size.height) / canvas.height}
			label={`Resize ${item.label} ${handle.name}`}
			data-matrix-layer-handle={kind === 'Layer' ? 'Resize' : undefined}
			data-matrix-layer-resize={kind === 'Layer' ? handle.name : undefined}
			data-matrix-cell-resize={kind === 'Cell' ? handle.name : undefined}
			style={`border-radius: 0.1rem; cursor: ${handle.cursor}`}
			{disabled}
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
		fill: transparent;
		stroke: var(--gc-color-border);
		stroke-width: 0.06rem;
		vector-effect: non-scaling-stroke;
		cursor: move;
	}
	rect.selected,
	rect:focus {
		stroke: var(--gc-color-focus);
		stroke-width: 0.12rem;
	}
	rect.inactive {
		stroke-dasharray: 0.3rem 0.2rem;
	}
</style>
