<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
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
	let canvas: SVGSVGElement;
	let pointer: number | null = null;
	let ghost = $state<Point | null>(null);
	const surface = $derived(
		screen.surfaces.find((surface) => surface.reference.surface === selected)
	);
	const visible = $derived(screen.surfaces.slice(0, 128));
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: (event.clientX - rect.left) / rect.width,
			y: (event.clientY - rect.top) / rect.height
		};
	}
	function down(event: PointerEvent, reference: SurfaceReference, corner: number) {
		if (disabled || event.button !== 0 || pointer !== null || !start(reference, corner)) return;
		event.preventDefault();
		event.stopPropagation();
		pointer = event.pointerId;
		ghost = point(event);
		canvas.setPointerCapture(pointer);
	}
	function release(event: PointerEvent, cancelled = false) {
		if (event.pointerId !== pointer) return;
		if (!cancelled) {
			const next = point(event);
			if (next) move(next);
			finish();
		} else stop();
		const previous = pointer;
		pointer = null;
		ghost = null;
		if (previous !== null && canvas.hasPointerCapture(previous))
			canvas.releasePointerCapture(previous);
	}
	function cancel() {
		if (pointer !== null) {
			const previous = pointer;
			pointer = null;
			ghost = null;
			if (canvas.hasPointerCapture(previous)) canvas.releasePointerCapture(previous);
			stop();
		}
	}
	onDestroy(cancel);
</script>

<svelte:window
	onkeydown={(event) => {
		if (event.key === 'Escape' && pointer !== null) {
			event.preventDefault();
			cancel();
		}
	}} />

<svg
	bind:this={canvas}
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	aria-label="Surface mapping controls"
	role="group"
	onpointermove={(event) => {
		if (event.pointerId === pointer) {
			ghost = point(event);
			if (ghost) move(ghost);
			else cancel();
		}
	}}
	onpointerup={(event) => release(event)}
	onpointercancel={(event) => release(event, true)}
	onlostpointercapture={cancel}>
	{#each visible as item (item.reference.surface)}
		<polygon
			points={item.definition.quad.corners.map((point) => `${point.x},${point.y}`).join(' ')}
			class:selected={item.reference.surface === selected}
			class:inactive={!item.definition.enabled}
			role="button"
			tabindex={disabled ? -1 : 0}
			aria-label={`Select ${item.label}`}
			aria-disabled={disabled}
			onclick={() => {
				if (!disabled) select(item.reference.surface);
			}}
			onkeydown={(event) => {
				if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
					event.preventDefault();
					select(item.reference.surface);
				}
			}} />
	{/each}
	{#if surface}
		{#each surface.definition.quad.corners as corner, index}
			<circle
				cx={corner.x}
				cy={corner.y}
				r="0.018"
				aria-hidden="true"
				onpointerdown={(event) => down(event, surface.reference, index)} />
		{/each}
	{/if}
	{#if ghost}<circle class="ghost" cx={ghost.x} cy={ghost.y} r="0.012" />{/if}
</svg>

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
	polygon,
	circle {
		pointer-events: all;
	}
	polygon {
		fill: #78aafa08;
		stroke: #a2b0c5;
		stroke-width: 0.003;
		cursor: pointer;
	}
	polygon.selected {
		fill: #78aafa12;
		stroke: #70aaff;
		stroke-width: 0.005;
	}
	polygon.inactive {
		stroke-dasharray: 0.015 0.01;
	}
	circle {
		fill: #dceaff;
		stroke: #467bcc;
		stroke-width: 0.003;
		cursor: crosshair;
	}
	circle.ghost {
		fill: none;
		stroke: #ffe19b;
		pointer-events: none;
	}
</style>
