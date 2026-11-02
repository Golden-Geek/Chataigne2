<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle } from 'golden_ui';
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
				data-instance-drop-target={item.reference.instance}
				data-source-drop-object={item.reference.instance}
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
	{/each}
	{#if instance && handles}
		{#if selectedExtent && !visible.some((item) => item.reference.instance === instance.reference.instance)}
			<polygon
				class="selected"
				data-instance-drop-target={instance.reference.instance}
				data-source-drop-object={instance.reference.instance}
				points={outline(instance.definition.transform, selectedExtent)
					.map((point) => `${point.x},${point.y}`)
					.join(' ')}
				aria-hidden="true"
				onpointerdown={(event) => down(event, instance!, 'Translate')} />
		{/if}
		<line x1={handles.pivot.x} y1={handles.pivot.y} x2={handles.rotate.x} y2={handles.rotate.y} />
		<line x1={handles.pivot.x} y1={handles.pivot.y} x2={handles.scale.x} y2={handles.scale.y} />
	{/if}
</svg>

{#each visible as item (item.reference.instance)}
	{#if item.reference.instance !== selected}
		<ViewportHandle
			x={item.definition.transform.position[0] / composition.canvas.width}
			y={item.definition.transform.position[1] / composition.canvas.height}
			label={`Move ${item.label}`}
			data-instance-drop-target={item.reference.instance}
			data-source-drop-object={item.reference.instance}
			{disabled}
			onpointerdown={(event) => down(event, item, 'Translate')} />
	{/if}
{/each}
{#if instance && handles}
	<ViewportHandle
		x={handles.pivot.x / composition.canvas.width}
		y={handles.pivot.y / composition.canvas.height}
		label={`Move ${instance.label}`}
		data-instance-drop-target={instance.reference.instance}
		data-source-drop-object={instance.reference.instance}
		data-instance-handle="Translate"
		{disabled}
		onpointerdown={(event) => down(event, instance!, 'Translate')} />
	<ViewportHandle
		x={handles.rotate.x / composition.canvas.width}
		y={handles.rotate.y / composition.canvas.height}
		label={`Rotate ${instance.label}`}
		data-instance-drop-target={instance.reference.instance}
		data-source-drop-object={instance.reference.instance}
		data-instance-handle="Rotate"
		{disabled}
		onpointerdown={(event) => down(event, instance!, 'Rotate')} />
	<ViewportHandle
		x={handles.scale.x / composition.canvas.width}
		y={handles.scale.y / composition.canvas.height}
		label={`Scale ${instance.label}`}
		data-instance-drop-target={instance.reference.instance}
		data-source-drop-object={instance.reference.instance}
		data-instance-handle="Scale"
		{disabled}
		onpointerdown={(event) => down(event, instance!, 'Scale')} />
{/if}
{#if ghost}<ViewportHandle
		x={ghost.x / composition.canvas.width}
		y={ghost.y / composition.canvas.height}
		label="Transform pointer"
		ghost />{/if}

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
	polygon {
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
</style>
