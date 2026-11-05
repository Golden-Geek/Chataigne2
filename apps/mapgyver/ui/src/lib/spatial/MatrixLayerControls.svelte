<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
	import type { MatrixViewRow } from '../media/generated/MatrixViewRow';
	import type { RectGeometryMode } from '../media/generated/RectGeometryMode';
	import type { Point } from '../media/generated/Point';
	import type { LayerHandle } from './layer-gesture';
	import { createSpatialPointer } from './pointer';
	import { SPATIAL_RESIZE_HANDLES } from './handles';
	let {
		rows,
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
		rows: MatrixViewRow[];
		canvas: { width: number; height: number };
		selected: string | null;
		disabled: boolean;
		select(uuid: string): void;
		active: boolean;
		start(layer: string, handle: LayerHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
		enter(layer: string): void;
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
	const item = $derived(rows.find((row) => row.item.id.id === selected)?.item);
	function down(event: PointerEvent, row: MatrixViewRow, mode: RectGeometryMode) {
		if (disabled || !row.item.layer) return;
		pointer.start(event, (at) => {
			const layer = row.item.layer!;
			if (
				!start(row.item.id.id, {
					origin: { position: [...layer.position], size: { ...layer.size } },
					mode,
					start: at
				})
			)
				return false;
			select(row.item.id.id);
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
	aria-label="Matrix Layer layout controls"
	onpointermove={pointer.move}
	onpointerup={(event) => pointer.release(event)}
	onpointercancel={(event) => pointer.release(event, true)}
	onlostpointercapture={pointer.cancel}>
	{#each rows as row (row.item.id.id)}
		{#if row.item.layer}
			<rect
				data-layer-id={row.item.id.id}
				x={row.item.layer.position[0]}
				y={row.item.layer.position[1]}
				width={row.item.layer.size.width}
				height={row.item.layer.size.height}
				class:selected={selected === row.item.id.id}
				class:inactive={!row.item.settings.enabled}
				role="button"
				tabindex={disabled ? -1 : 0}
				aria-label={`Select ${row.item.label}`}
				aria-disabled={disabled}
				onpointerdown={(event) => down(event, row, 'Translate')}
				ondblclick={(event) => {
					event.stopPropagation();
					enter(row.item.id.id);
				}}
				onclick={(event) => {
					event.stopPropagation();
					if (!disabled) select(row.item.id.id);
				}}
				onkeydown={(event) => {
					if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(row.item.id.id);
					}
				}} />
		{/if}
	{/each}
</svg>

{#if item?.layer}
	{#each SPATIAL_RESIZE_HANDLES as handle (handle.name)}
		<ViewportHandle
			x={(item.layer.position[0] + handle.x * item.layer.size.width) / canvas.width}
			y={(item.layer.position[1] + handle.y * item.layer.size.height) / canvas.height}
			label={`Resize ${item.label} ${handle.name}`}
			data-matrix-layer-handle="Resize"
			data-matrix-layer-resize={handle.name}
			style={`border-radius: 0.1rem; cursor: ${handle.cursor}`}
			{disabled}
			onpointerdown={(event) => {
				const row = rows.find((row) => row.item.id.id === selected);
				if (row) down(event, row, { Resize: { handle: handle.name } });
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
