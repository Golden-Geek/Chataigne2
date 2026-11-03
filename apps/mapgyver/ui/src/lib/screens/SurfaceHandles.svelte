<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { SurfaceId } from './generated/SurfaceId';
	import type { SurfaceReference } from './generated/SurfaceReference';
	import type { Point } from './generated/Point';
	import type { SurfaceHandle } from './surface-gesture';
	type WarpHandle = Extract<SurfaceHandle, { kind: 'QuadCorner' | 'QuadTranslation' }>;
	let {
		screen,
		selected,
		selectedCorner,
		pickCorner,
		disabled,
		select,
		start,
		move,
		finish,
		stop
	}: {
		screen: ScreenSnapshot;
		selected: SurfaceId | null;
		selectedCorner: number | null;
		pickCorner(surface: SurfaceId, corner: number): void;
		disabled: boolean;
		select(id: SurfaceId | null): void;
		start(reference: SurfaceReference, handle: WarpHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	let canvas: SVGSVGElement;
	let pointer = $state.raw<{
		id: number;
		reference: SurfaceReference;
		handle: WarpHandle;
		x: number;
		y: number;
		threshold: number;
		editing: boolean;
	} | null>(null);
	let ghost = $state<Point | null>(null);
	let hovered = $state<SurfaceId | null>(null);
	const visible = $derived(screen.surfaces.slice(0, 128));
	const handles = $derived(
		[selected, hovered].flatMap((id, index, all) => {
			if (!id || all.indexOf(id) !== index) return [];
			const item = screen.surfaces.find((item) => item.reference.surface === id);
			return item ? [item] : [];
		})
	);
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: (event.clientX - rect.left) / rect.width,
			y: (event.clientY - rect.top) / rect.height
		};
	}
	function down(event: PointerEvent, reference: SurfaceReference, corner: number | null) {
		if (disabled || event.button !== 0 || pointer !== null || (!event.altKey && corner === null))
			return;
		const origin = point(event);
		const surface = screen.surfaces.find((item) => item.reference.surface === reference.surface);
		if (!origin || !surface) return;
		let handle: WarpHandle;
		if (event.altKey)
			handle = {
				kind: 'QuadTranslation',
				origin,
				quad: {
					corners: surface.definition.quad.corners.map((point) => ({
						...point
					})) as typeof surface.definition.quad.corners
				}
			};
		else if (corner !== null) handle = { kind: 'QuadCorner', corner };
		else return;
		event.preventDefault();
		event.stopPropagation();
		if (corner === null) select(reference.surface);
		else pickCorner(reference.surface, corner);
		// Selection must not open an authored edit until the pointer actually drags.
		pointer = {
			id: event.pointerId,
			reference,
			handle,
			x: event.clientX,
			y: event.clientY,
			threshold:
				(Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16) * 0.15,
			editing: false
		};
		canvas.setPointerCapture(pointer.id);
	}
	function update(event: PointerEvent) {
		const active = pointer;
		if (!active || event.pointerId !== active.id) return;
		if (!active.editing) {
			if (Math.hypot(event.clientX - active.x, event.clientY - active.y) < active.threshold) return;
			if (!start(active.reference, active.handle)) {
				cancel();
				return;
			}
			active.editing = true;
		}
		ghost = point(event);
		if (ghost) move(ghost);
		else cancel();
	}
	function release(event: PointerEvent, cancelled = false) {
		const active = pointer;
		if (!active || event.pointerId !== active.id) return;
		if (active.editing) {
			if (!cancelled) {
				update(event);
				if (pointer) finish();
			} else stop();
		}
		pointer = null;
		ghost = null;
		if (canvas.hasPointerCapture(active.id)) canvas.releasePointerCapture(active.id);
	}
	function cancel() {
		const active = pointer;
		if (!active) return;
		pointer = null;
		ghost = null;
		if (canvas.hasPointerCapture(active.id)) canvas.releasePointerCapture(active.id);
		if (active.editing) stop();
	}
	function leave(event: PointerEvent, id: SurfaceId) {
		const next =
			event.relatedTarget instanceof Element
				? event.relatedTarget.closest('[data-surface-drop-target]')
				: null;
		if (next?.getAttribute('data-surface-drop-target') === id) return;
		if (hovered === id) hovered = null;
	}
	$effect(() => {
		const current = selected;
		if (pointer && pointer.reference.surface !== current) cancel();
	});
	onDestroy(cancel);
</script>

<svelte:window
	onblur={cancel}
	onkeydown={(event) => {
		if (event.key === 'Escape' && pointer !== null) {
			event.preventDefault();
			cancel();
		}
	}} />

<!-- Captured handle clicks belong to this control group, including after release. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<svg
	bind:this={canvas}
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	aria-label="Surface mapping controls"
	role="group"
	onclick={(event) => event.stopPropagation()}
	onkeydown={(event) => {
		if (event.key === 'Escape') cancel();
	}}
	onpointermove={update}
	onpointerup={(event) => release(event)}
	onpointercancel={(event) => release(event, true)}
	onlostpointercapture={cancel}>
	{#each visible as item (item.reference.surface)}
		<polygon
			data-surface-drop-target={item.reference.surface}
			data-source-drop-object={item.reference.surface}
			points={item.definition.quad.corners.map((point) => `${point.x},${point.y}`).join(' ')}
			class:selected={item.reference.surface === selected}
			class:inactive={!item.definition.enabled}
			class:invalid={!item.mapping_valid}
			role="button"
			tabindex={disabled ? -1 : 0}
			aria-label={`Select ${item.label}${item.mapping_valid ? '' : ' (invalid warp shape)'}`}
			aria-disabled={disabled}
			onpointerdown={(event) => down(event, item.reference, null)}
			onpointerenter={() => {
				hovered = item.reference.surface;
			}}
			onpointerleave={(event) => leave(event, item.reference.surface)}
			onclick={(event) => {
				event.stopPropagation();
				if (!disabled) select(item.reference.surface);
			}}
			onkeydown={(event) => {
				if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
					event.preventDefault();
					event.stopPropagation();
					select(item.reference.surface);
				}
			}}><title>Alt-drag to move Surface</title></polygon>
	{/each}
</svg>

{#each handles as item (item.reference.surface)}
	<div class="corners" role="group" aria-label={`Warp handles for ${item.label}`}>
		{#each item.definition.quad.corners as corner, index}
			<ViewportHandle
				x={corner.x}
				y={corner.y}
				label={`Warp corner ${index + 1}`}
				title="Drag corner; Alt-drag to move Surface"
				selected={item.reference.surface === selected && index === selectedCorner}
				data-surface-drop-target={item.reference.surface}
				data-source-drop-object={item.reference.surface}
				{disabled}
				onpointerenter={() => {
					hovered = item.reference.surface;
				}}
				onpointerleave={(event) => leave(event, item.reference.surface)}
				onpointerdown={(event) => down(event, item.reference, index)}
				onclick={(event) => {
					event.stopPropagation();
					if (!disabled && (item.reference.surface !== selected || index !== selectedCorner))
						pickCorner(item.reference.surface, index);
				}}
				onkeydown={(event) => {
					if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						event.stopPropagation();
						pickCorner(item.reference.surface, index);
					}
				}} />
		{/each}
	</div>
{/each}
{#if ghost}<ViewportHandle x={ghost.x} y={ghost.y} label="Warp pointer" ghost />{/if}

<style>
	.corners {
		position: absolute;
		inset: 0;
		pointer-events: none;
	}
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: visible;
		touch-action: none;
		pointer-events: none;
	}
	polygon {
		pointer-events: all;
	}
	polygon {
		fill: #78aafa08;
		stroke: #a2b0c5;
		stroke-width: 0.1rem;
		vector-effect: non-scaling-stroke;
		cursor: pointer;
	}
	polygon.selected {
		fill: #78aafa12;
		stroke: #70aaff;
	}
	polygon.inactive {
		stroke-dasharray: 0.3rem 0.2rem;
	}
	polygon.invalid {
		fill: #a51d2c55;
		stroke: #ff5364;
	}
</style>
