<script lang="ts">
	import { onDestroy } from 'svelte';
	import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
	import type { InstanceId } from '../media/generated/InstanceId';
	import type { InstanceReference } from '../media/generated/InstanceReference';
	import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
	import type { InstanceGeometryMode } from '../media/generated/InstanceGeometryMode';
	import type { Point } from '../media/generated/Point';
	import type { InstanceHandle } from './instance-gesture';
	import { usePreview } from '../preview/context.svelte';
	import { previewKey } from '../preview/identity';
	import { outline, gizmo } from './presentation';
	let {
		composition,
		selected,
		disabled,
		active,
		select,
		start,
		move,
		finish,
		stop
	}: {
		composition: CompositionSnapshot;
		selected: InstanceId | null;
		disabled: boolean;
		active: boolean;
		select(id: InstanceId): void;
		start(reference: InstanceReference, handle: InstanceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	const owner = usePreview();
	const instance = $derived(
		composition.instances.find((item) => item.reference.instance === selected)
	);
	const visible = $derived(composition.instances.slice(0, 128));
	const selectedExtent = $derived(
		instance?.definition.source
			? owner.statuses.get(previewKey({ kind: 'Channel', reference: instance.definition.source }))
					?.extent
			: null
	);
	const handles = $derived(
		instance ? gizmo(instance.definition.transform, composition.canvas, selectedExtent) : null
	);
	let canvas: SVGSVGElement;
	let pointer: number | null = null;
	let ghost = $state<Point | null>(null);
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: ((event.clientX - rect.left) / rect.width) * composition.canvas.width,
			y: ((event.clientY - rect.top) / rect.height) * composition.canvas.height
		};
	}
	function down(event: PointerEvent, item: InstanceSnapshot, mode: InstanceGeometryMode) {
		const at = point(event);
		if (disabled || event.button !== 0 || pointer !== null || !at) return;
		const handle: InstanceHandle = {
			mode,
			origin: {
				...item.definition.transform,
				position: [...item.definition.transform.position],
				scale: [...item.definition.transform.scale],
				pivot: [...item.definition.transform.pivot]
			},
			start: at
		};
		if (!start(item.reference, handle)) return;
		select(item.reference.instance);
		event.preventDefault();
		event.stopPropagation();
		pointer = event.pointerId;
		ghost = at;
		canvas.setPointerCapture(pointer);
	}
	function cancel() {
		if (pointer === null) return;
		const previous = pointer;
		pointer = null;
		ghost = null;
		if (canvas.hasPointerCapture(previous)) canvas.releasePointerCapture(previous);
		stop();
	}
	function release(event: PointerEvent, cancelled = false) {
		if (event.pointerId !== pointer) return;
		if (cancelled) {
			cancel();
			return;
		}
		const next = point(event);
		if (next) move(next);
		finish();
		const previous = pointer;
		pointer = null;
		ghost = null;
		if (previous !== null && canvas.hasPointerCapture(previous))
			canvas.releasePointerCapture(previous);
	}
	$effect(() => {
		if (!active) cancel();
	});
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
	viewBox={`0 0 ${composition.canvas.width} ${composition.canvas.height}`}
	preserveAspectRatio="none"
	aria-label="Composition transform controls"
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
	{#each visible as item (item.reference.instance)}
		{@const extent = item.definition.source
			? owner.statuses.get(previewKey({ kind: 'Channel', reference: item.definition.source }))
					?.extent
			: null}
		{#if extent}
			<polygon
				points={outline(item.definition.transform, extent)
					.map((point) => `${point.x},${point.y}`)
					.join(' ')}
				class:selected={item.reference.instance === selected}
				class:inactive={!item.definition.enabled}
				role="button"
				tabindex={disabled ? -1 : 0}
				aria-label={`Select ${item.label}`}
				aria-disabled={disabled}
				onpointerdown={(event) => down(event, item, 'Translate')}
				onclick={() => {
					if (!disabled) select(item.reference.instance);
				}}
				onkeydown={(event) => {
					if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(item.reference.instance);
					}
				}} />
		{/if}
		<circle
			cx={item.definition.transform.position[0]}
			cy={item.definition.transform.position[1]}
			r={Math.min(composition.canvas.width, composition.canvas.height) * 0.008}
			class="pivot"
			aria-hidden="true"
			onpointerdown={(event) => down(event, item, 'Translate')} />
	{/each}
	{#if instance && handles}
		{#if selectedExtent && !visible.some((item) => item.reference.instance === instance.reference.instance)}
			<polygon
				class="selected"
				points={outline(instance.definition.transform, selectedExtent)
					.map((point) => `${point.x},${point.y}`)
					.join(' ')}
				aria-hidden="true"
				onpointerdown={(event) => down(event, instance!, 'Translate')} />
		{/if}
		<line x1={handles.pivot.x} y1={handles.pivot.y} x2={handles.rotate.x} y2={handles.rotate.y} />
		<line x1={handles.pivot.x} y1={handles.pivot.y} x2={handles.scale.x} y2={handles.scale.y} />
		<circle
			cx={handles.pivot.x}
			cy={handles.pivot.y}
			r={handles.radius * 1.4}
			class="selected-pivot"
			aria-hidden="true"
			onpointerdown={(event) => down(event, instance!, 'Translate')} />
		<circle
			cx={handles.rotate.x}
			cy={handles.rotate.y}
			r={handles.radius}
			class="rotate"
			aria-hidden="true"
			onpointerdown={(event) => down(event, instance!, 'Rotate')} />
		<rect
			x={handles.scale.x - handles.radius}
			y={handles.scale.y - handles.radius}
			width={handles.radius * 2}
			height={handles.radius * 2}
			class="scale"
			aria-hidden="true"
			onpointerdown={(event) => down(event, instance!, 'Scale')} />
	{/if}
	{#if ghost}<circle
			class="ghost"
			cx={ghost.x}
			cy={ghost.y}
			r={Math.min(composition.canvas.width, composition.canvas.height) * 0.006} />{/if}
</svg>

<style>
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: hidden;
		touch-action: none;
		pointer-events: none;
	}
	polygon,
	circle,
	rect {
		pointer-events: all;
	}
	polygon {
		fill: #78aafa08;
		stroke: #a2b0c5;
		stroke-width: 0.1rem;
		vector-effect: non-scaling-stroke;
		cursor: move;
	}
	polygon.selected {
		fill: #78aafa12;
		stroke: #70aaff;
	}
	polygon.inactive {
		stroke-dasharray: 0.3rem 0.2rem;
	}
	line {
		stroke: #70aaff;
		stroke-width: 0.1rem;
		vector-effect: non-scaling-stroke;
	}
	circle,
	rect {
		fill: #dceaff;
		stroke: #467bcc;
		stroke-width: 0.1rem;
		vector-effect: non-scaling-stroke;
	}
	.pivot,
	.selected-pivot {
		cursor: move;
	}
	.rotate {
		cursor: crosshair;
		fill: #ffe19b;
	}
	.scale {
		cursor: nwse-resize;
	}
	.ghost {
		fill: none;
		stroke: #ffe19b;
		pointer-events: none;
	}
</style>
