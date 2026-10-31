<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { UnitRect } from './generated/UnitRect';
	import type { Point } from './generated/Point';
	import type { SurfaceHandle } from './surface-gesture';
	let {
		crop,
		disabled,
		start,
		move,
		finish,
		stop
	}: {
		crop: UnitRect;
		disabled: boolean;
		start(handle: SurfaceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	let canvas: SVGSVGElement;
	let drag: { id: number; offset: Point } | null = null;
	let ghost = $state<Point | null>(null);
	const corners = $derived([
		{ x: crop.x, y: crop.y },
		{ x: crop.x + crop.width, y: crop.y },
		{ x: crop.x + crop.width, y: crop.y + crop.height },
		{ x: crop.x, y: crop.y + crop.height }
	]);
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		return rect.width > 0 && rect.height > 0
			? { x: (event.clientX - rect.left) / rect.width, y: (event.clientY - rect.top) / rect.height }
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
		canvas.setPointerCapture(drag.id);
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
	bind:this={canvas}
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	role="group"
	aria-label="Source crop controls"
	onpointermove={update}
	onpointerup={release}
	onpointercancel={(event) => {
		if (event.pointerId === drag?.id) cancel();
	}}
	onlostpointercapture={(event) => {
		if (event.pointerId === drag?.id) cancel();
	}}>
	<rect
		x={crop.x}
		y={crop.y}
		width={crop.width}
		height={crop.height}
		aria-hidden="true"
		onpointerdown={(event) => down(event, { kind: 'CropOrigin' })} />
	{#each corners as corner, index}
		<circle
			cx={corner.x}
			cy={corner.y}
			r="0.018"
			aria-hidden="true"
			onpointerdown={(event) => down(event, { kind: 'CropCorner', corner: index })} />
	{/each}
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
	rect {
		fill: #78aafa12;
		stroke: #70aaff;
		stroke-width: 0.004;
		cursor: move;
		pointer-events: all;
	}
	circle {
		fill: #dceaff;
		stroke: #467bcc;
		stroke-width: 0.003;
		cursor: crosshair;
		pointer-events: all;
	}
	circle.ghost {
		fill: none;
		stroke: #ffe19b;
		pointer-events: none;
	}
</style>
