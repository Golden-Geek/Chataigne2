<script lang="ts">
	import { onDestroy } from 'svelte';
	import {
		ViewportHandle,
		ViewportTarget,
		ViewportGuides,
		pointBounds,
		viewportContour,
		type AlignmentGuide
	} from 'golden_ui';
	import { createGeometryAlignment } from '../spatial/alignment';
	import type { CompositionSnapshot } from '../media/generated/CompositionSnapshot';
	import type { InstanceId } from '../media/generated/InstanceId';
	import type { InstanceReference } from '../media/generated/InstanceReference';
	import type { InstanceSnapshot } from '../media/generated/InstanceSnapshot';
	import type { ChannelReference } from '../media/generated/ChannelReference';
	import type { Extent2D } from '../media/generated/Extent2D';
	import type { InstanceGeometryMode } from '../media/generated/InstanceGeometryMode';
	import type { Point } from '../media/generated/Point';
	import type { InstanceHandle, InstancePresentation } from './instance-gesture';
	import { instanceDisplay } from './presentation';
	import rotateCursor from './rotate-cursor.svg';
	import { createViewportPointer } from 'golden_ui';
	import { customNodeColor } from '../node-color.svelte';
	import { spatialOverlays, useSpatialPaint } from '../spatial/native-paint';
	let {
		composition,
		sourceExtent: readExtent,
		snapping = false,
		presentation = null,
		selected,
		disabled,
		selectionAvailable,
		active,
		select,
		start,
		move,
		finish,
		stop
	}: {
		composition: CompositionSnapshot;
		sourceExtent(reference: ChannelReference | null): Extent2D | null;
		snapping?: boolean;
		presentation?: InstancePresentation | null;
		selected: InstanceId | null;
		disabled: boolean;
		selectionAvailable: boolean;
		active: boolean;
		select(id: InstanceId): void;
		start(reference: InstanceReference, handle: InstanceHandle): boolean;
		move(point: Point): void;
		finish(): void;
		stop(): void;
	} = $props();
	const instance = $derived(
		composition.instances.find((item) => item.reference.instance === selected)
	);
	// The bounded page plus its independent selection contains at most 129 items.
	const visible = $derived(composition.instances.toReversed());
	const displayed = $derived(
		visible.map((item) => {
			const extent = readExtent(item.definition.source);
			return { item, points: extent ? instanceDisplay(item, extent, presentation).points : null };
		})
	);
	const selectedExtent = $derived(readExtent(instance?.definition.source ?? null));
	const handles = $derived(
		instance ? instanceDisplay(instance, selectedExtent, presentation).handles : null
	);
	let canvas: SVGSVGElement;
	const paint = useSpatialPaint();
	let hovered = $state<string | null>(null);
	let focused = $state<string | null>(null);
	let ghost = $state<Point | null>(null);
	let guides = $state<AlignmentGuide[]>([]);
	$effect(() => {
		if (!paint) return;
		const contours = displayed.flatMap(({ item, points }) => {
			if (!points) return [];
			const id = item.reference.instance;
			return [
				{
					hit: id,
					points,
					color: customNodeColor(id),
					selected: id === selected,
					enabled: item.definition.enabled,
					hovered: hovered === id || focused === id
				}
			];
		});
		const points = handles ? [handles.pivot, ...handles.resize.map((handle) => handle.point)] : [];
		paint(spatialOverlays(composition.canvas, contours, points, ghost, guides));
	});
	const alignment = createGeometryAlignment(
		() => canvas,
		() => snapping,
		(next) => {
			guides = next;
		}
	);
	function sourceExtent(item: InstanceSnapshot) {
		return readExtent(item.definition.source);
	}
	const pointer = createViewportPointer({
		canvas: () => canvas,
		extent: () => composition.canvas,
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
	function down(event: PointerEvent, item: InstanceSnapshot, mode: InstanceGeometryMode) {
		pointer.start(event, {
			selectable: selectionAvailable && !active,
			editable: !disabled,
			select: () => select(item.reference.instance),
			begin: (at) => {
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
				if (!start(item.reference, handle)) return false;
				if (mode === 'Rotate') alignment.clear();
				else {
					const extent = sourceExtent(item);
					alignment.begin(
						composition.instances.map((entry) => ({
							id: entry.reference.instance,
							bounds: pointBounds(instanceDisplay(entry, sourceExtent(entry), null).points)
						})),
						item.reference.instance,
						composition.canvas,
						(point) =>
							pointBounds(
								instanceDisplay(item, extent, { reference: item.reference, handle, sample: point })
									.points
							),
						{ centers: mode === 'Translate' }
					);
				}
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
	bind:this={canvas}
	viewBox={`0 0 ${composition.canvas.width} ${composition.canvas.height}`}
	preserveAspectRatio="none"
	aria-label="Composition transform controls"
	data-geometry-presentation={presentation ? 'pending' : 'committed'}
	role="group"
	onpointermove={pointer.move}
	onpointerup={(event) => pointer.release(event)}
	onpointercancel={(event) => pointer.release(event, true)}
	onlostpointercapture={pointer.cancel}>
	{#each displayed as { item, points } (item.reference.instance)}
		{#if points}
			<polygon
				use:viewportContour={{
					color: customNodeColor(item.reference.instance),
					selected: item.reference.instance === selected,
					enabled: item.definition.enabled,
					identity: composition.reference.channel,
					onhover: (value) => {
						if (value || hovered === item.reference.instance)
							hovered = value ? item.reference.instance : null;
					},
					onfocus: (value) => {
						if (value || focused === item.reference.instance)
							focused = value ? item.reference.instance : null;
					}
				}}
				data-native-overlay-control
				data-instance-drop-target={item.reference.instance}
				data-source-drop-object={item.reference.instance}
				points={points.map((point) => `${point.x},${point.y}`).join(' ')}
				class:selected={item.reference.instance === selected}
				class:inactive={!item.definition.enabled}
				role="button"
				tabindex={selectionAvailable ? 0 : -1}
				aria-label={`Select ${item.label}`}
				aria-disabled={!selectionAvailable || active}
				onpointerdown={(event) => down(event, item, 'Translate')}
				onclick={(event) => {
					if (selectionAvailable && !active && event.detail === 0) select(item.reference.instance);
				}}
				onkeydown={(event) => {
					if (selectionAvailable && !active && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						select(item.reference.instance);
					}
				}} />
		{/if}
	{/each}
</svg>
<ViewportGuides
	{guides}
	width={composition.canvas.width}
	height={composition.canvas.height}
	native={!!paint} />

{#each visible as item (item.reference.instance)}
	{@const extent = sourceExtent(item)}
	{#if item.reference.instance !== selected && !extent}
		<ViewportTarget
			identity={composition.reference.channel}
			enabled={item.definition.enabled}
			x={item.definition.transform.position[0] / composition.canvas.width}
			y={item.definition.transform.position[1] / composition.canvas.height}
			label={`Select ${item.label}`}
			data-instance-drop-target={item.reference.instance}
			data-source-drop-object={item.reference.instance}
			aria-disabled={!selectionAvailable || active}
			onpointerdown={(event) => down(event, item, 'Translate')}
			onclick={(event) => {
				if (selectionAvailable && !active && event.detail === 0) select(item.reference.instance);
			}} />
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
		data-native-overlay-control
		aria-disabled={disabled && !active}
		onpointerdown={(event) => down(event, instance!, 'Translate')} />
	{#each handles.resize as handle (handle.name)}
		<ViewportHandle
			x={handle.point.x / composition.canvas.width}
			y={handle.point.y / composition.canvas.height}
			label={`Resize ${instance.label} ${handle.name}`}
			tabindex={disabled ? -1 : 0}
			data-instance-handle="Resize"
			data-instance-resize={handle.name}
			data-native-overlay-control
			style={`border-radius: 0.1rem; cursor: ${handle.cursor}`}
			aria-disabled={disabled && !active}
			onpointerdown={(event) => {
				if (selectedExtent)
					down(event, instance!, { Resize: { handle: handle.name, extent: selectedExtent } });
			}} />
	{/each}
	{#each handles.rotate as handle (handle.name)}
		<ViewportHandle
			x={handle.point.x / composition.canvas.width}
			y={handle.point.y / composition.canvas.height}
			label={`Rotate ${instance.label} ${handle.name}`}
			tabindex={disabled ? -1 : 0}
			data-instance-handle="Rotate"
			paint={false}
			data-instance-rotation={handle.name}
			style={`width: 1.2rem; height: 1.2rem; background: transparent; border: none; transform: translate(-50%, -50%) translate(${handle.offset.x * 1.3}rem, ${handle.offset.y * 1.3}rem); cursor: url(${JSON.stringify(rotateCursor)}) 12 12, crosshair`}
			aria-disabled={disabled && !active}
			onpointerdown={(event) => down(event, instance!, 'Rotate')} />
	{/each}
{/if}
{#if ghost}<ViewportHandle
		x={ghost.x / composition.canvas.width}
		y={ghost.y / composition.canvas.height}
		label="Transform pointer"
		data-native-overlay-control
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
		cursor: move;
	}
</style>
