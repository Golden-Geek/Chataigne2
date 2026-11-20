<script lang="ts">
	import { onDestroy } from 'svelte';
	import { ViewportHandle, viewportContour } from 'golden_ui';
	import { capturePreviewPointer, type OverlayPrimitive } from 'golden_video_ui';
	import { handleOverlays, imageFrameOverlay, overlayRem, quadOverlay } from '../preview/overlays';
	import type { ScreenSnapshot } from './generated/ScreenSnapshot';
	import type { SurfaceId } from './generated/SurfaceId';
	import type { SurfaceReference } from './generated/SurfaceReference';
	import type { SurfaceSnapshot } from './generated/SurfaceSnapshot';
	import type { Point } from './generated/Point';
	import type { SurfaceHandle } from './surface-gesture';
	import type { WarpOutlines } from './gesture-presentation';
	import { nearestHandle } from './handle-snap';
	import { linkOverlays } from './link-overlays';
	import { customNodeColor } from '../node-color.svelte';
	import type { ScreenHandleCandidate } from './generated/ScreenHandleCandidate';
	import type { ScreenHandleGroup } from './generated/ScreenHandleGroup';
	import type { CornerLinkEdit } from './warp-edit-group';
	type WarpHandle = Extract<SurfaceHandle, { kind: 'QuadCorner' | 'QuadTranslation' }>;
	let {
		snapping = false,
		candidates = [],
		groups = [],
		unlink = () => {},
		unlinkDisabled = false,
		screen,
		outlines,
		presenting,
		selected,
		selectedCorner,
		pickCorner,
		disabled,
		selectionAvailable,
		select,
		start,
		move,
		finish,
		stop,
		onoverlays = () => {}
	}: {
		snapping?: boolean;
		candidates?: ScreenHandleCandidate[];
		groups?: ScreenHandleGroup[];
		unlink?(corner: number): void;
		unlinkDisabled?: boolean;
		screen: ScreenSnapshot;
		outlines: WarpOutlines;
		presenting: boolean;
		selected: SurfaceId | null;
		selectedCorner: number | null;
		pickCorner(surface: SurfaceId, corner: number): void;
		disabled: boolean;
		selectionAvailable: boolean;
		select(id: SurfaceId | null): void;
		start(reference: SurfaceReference, handle: WarpHandle): boolean;
		move(point: Point): void;
		finish(link?: CornerLinkEdit | null): void;
		stop(): void;
		onoverlays?(overlays: OverlayPrimitive[]): void;
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
	let snapped = $state.raw<ScreenHandleCandidate | null>(null);
	let frame = $state({ width: 1, height: 1 });
	$effect(() => {
		if (!canvas) return;
		const element = canvas;
		const update = () => {
			const rect = element.getBoundingClientRect();
			frame = { width: rect.width, height: rect.height };
		};
		update();
		const observer = new ResizeObserver(update);
		observer.observe(element);
		return () => observer.disconnect();
	});
	let hovered = $state<SurfaceId | null>(null);
	let focusedSurface = $state<SurfaceId | null>(null);
	let focusedHandle = $state<string | null>(null);
	const visible = $derived(screen.surfaces.slice(0, 128));
	const corners = (surface: SurfaceSnapshot) =>
		outlines.get(surface.reference.surface) ?? surface.definition.quad.corners;
	const handles = $derived(
		[selected].flatMap((id, index, all) => {
			if (!id || all.indexOf(id) !== index) return [];
			const item = screen.surfaces.find((item) => item.reference.surface === id);
			return item ? [item] : [];
		})
	);
	$effect(() => {
		const rem = overlayRem();
		onoverlays([
			imageFrameOverlay(rem, customNodeColor(screen.id)),
			...visible.map((item) =>
				quadOverlay(corners(item), rem, {
					color: customNodeColor(item.reference.surface),
					hit: item.reference.surface,
					selected: item.reference.surface === selected,
					hovered: item.reference.surface === hovered || item.reference.surface === focusedSurface,
					enabled: item.definition.enabled,
					valid: item.mapping_valid
				})
			),
			...handles.flatMap((item) =>
				corners(item).flatMap((point, index) =>
					handleOverlays(
						point,
						rem,
						(item.reference.surface === selected && index === selectedCorner) ||
							focusedHandle === `${item.reference.surface}:${index}`,
						false,
						`${item.reference.surface}:${index}`
					)
				)
			),
			...(ghost ? handleOverlays(ghost, rem, false, true) : []),
			...handles.flatMap((item) =>
				groups.flatMap((group) => {
					const point = corners(item)[group.corner];
					return point
						? linkOverlays(
								point,
								frame,
								rem,
								`link:${item.reference.surface}:${group.corner}`,
								group.missing
							)
						: [];
				})
			)
		]);
	});
	function point(event: PointerEvent): Point | null {
		const rect = canvas.getBoundingClientRect();
		if (rect.width <= 0 || rect.height <= 0) return null;
		return {
			x: (event.clientX - rect.left) / rect.width,
			y: (event.clientY - rect.top) / rect.height
		};
	}
	function down(event: PointerEvent, reference: SurfaceReference, corner: number | null) {
		if (!selectionAvailable || event.button !== 0 || pointer !== null) return;
		event.preventDefault();
		event.stopPropagation();
		if (corner === null) select(reference.surface);
		else pickCorner(reference.surface, corner);
		// A pending editor read may block geometry without blocking document selection.
		if (disabled || (corner === null && !event.altKey)) return;
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
		capturePreviewPointer(canvas, event);
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
		const position = point(event);
		const rect = canvas.getBoundingClientRect();
		snapped =
			position && snapping && active.handle.kind === 'QuadCorner'
				? nearestHandle(position, active.handle.corner, candidates, rect, overlayRem() * 0.65)
				: null;
		ghost = snapped?.point ?? position;
		if (ghost) move(ghost);
		else cancel();
	}
	function release(event: PointerEvent, cancelled = false) {
		const active = pointer;
		if (!active || event.pointerId !== active.id) return;
		if (active.editing) {
			if (!cancelled) {
				update(event);
				if (pointer)
					finish(
						snapped && active.handle.kind === 'QuadCorner'
							? {
									kind: 'LinkScreenCorners',
									source: { reference: active.reference, corner: active.handle.corner },
									target: snapped.member
								}
							: null
					);
			} else stop();
		}
		pointer = null;
		ghost = null;
		snapped = null;
		if (canvas.hasPointerCapture(active.id)) canvas.releasePointerCapture(active.id);
	}
	function cancel() {
		const active = pointer;
		if (!active) return;
		pointer = null;
		ghost = null;
		snapped = null;
		if (canvas.hasPointerCapture(active.id)) canvas.releasePointerCapture(active.id);
		if (active.editing) stop();
	}
	$effect(() => {
		const current = selected;
		if (pointer && (!selectionAvailable || pointer.reference.surface !== current)) cancel();
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
	data-native-overlay-control
	bind:this={canvas}
	viewBox="0 0 1 1"
	preserveAspectRatio="none"
	aria-label="Surface mapping controls"
	data-geometry-presentation={presenting ? 'pending' : 'committed'}
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
			use:viewportContour={{
				color: customNodeColor(item.reference.surface),
				selected: item.reference.surface === selected,
				enabled: item.definition.enabled,
				valid: item.mapping_valid,
				onhover: (value) => {
					if (value) hovered = item.reference.surface;
					else if (hovered === item.reference.surface) hovered = null;
				},
				onfocus: (value) => {
					if (value) focusedSurface = item.reference.surface;
					else if (focusedSurface === item.reference.surface) focusedSurface = null;
				}
			}}
			data-native-hit-id={item.reference.surface}
			data-surface-drop-target={item.reference.surface}
			data-source-drop-object={item.reference.surface}
			points={corners(item)
				.map((point) => `${point.x},${point.y}`)
				.join(' ')}
			class:selected={item.reference.surface === selected}
			class:inactive={!item.definition.enabled}
			class:invalid={!item.mapping_valid}
			role="button"
			tabindex={selectionAvailable ? 0 : -1}
			aria-label={`Select ${item.label}${item.mapping_valid ? '' : ' (invalid warp shape)'}`}
			aria-disabled={!selectionAvailable}
			onpointerdown={(event) => down(event, item.reference, null)}
			onclick={(event) => {
				event.stopPropagation();
				if (selectionAvailable && event.detail === 0) select(item.reference.surface);
			}}
			onkeydown={(event) => {
				if (selectionAvailable && (event.key === 'Enter' || event.key === ' ')) {
					event.preventDefault();
					event.stopPropagation();
					select(item.reference.surface);
				}
			}}><title>Alt-drag to move Surface</title></polygon>
	{/each}
</svg>

{#each handles as item (item.reference.surface)}
	<div class="corners" role="group" aria-label={`Warp handles for ${item.label}`}>
		{#each corners(item) as corner, index}
			<ViewportHandle
				data-native-hit-id={`${item.reference.surface}:${index}`}
				data-native-overlay-control
				x={corner.x}
				y={corner.y}
				label={`Warp corner ${index + 1}`}
				tabindex={selectionAvailable ? 0 : -1}
				onfocus={() => {
					focusedHandle = `${item.reference.surface}:${index}`;
				}}
				onblur={() => {
					focusedHandle = null;
				}}
				title="Drag corner; Alt-drag to move Surface"
				selected={item.reference.surface === selected && index === selectedCorner}
				data-surface-drop-target={item.reference.surface}
				data-source-drop-object={item.reference.surface}
				disabled={!selectionAvailable}
				onpointerdown={(event) => down(event, item.reference, index)}
				onclick={(event) => {
					event.stopPropagation();
					if (
						selectionAvailable &&
						event.detail === 0 &&
						(item.reference.surface !== selected || index !== selectedCorner)
					)
						pickCorner(item.reference.surface, index);
				}}
				onkeydown={(event) => {
					if (selectionAvailable && (event.key === 'Enter' || event.key === ' ')) {
						event.preventDefault();
						event.stopPropagation();
						pickCorner(item.reference.surface, index);
					}
				}} />
		{/each}
		{#each groups as group (group.corner)}
			{@const point = corners(item)[group.corner]}
			{#if point}
				<button
					type="button"
					class="corner-link"
					data-native-overlay-control
					data-native-hit-id={`link:${item.reference.surface}:${group.corner}`}
					style:left={`calc(${point.x * 100}% + 1.2rem)`}
					style:top={`${point.y * 100}%`}
					aria-label={`Unlink corner group ${group.corner + 1}`}
					title={group.missing
						? 'Unlink missing corner target'
						: `Unlink all ${group.members.length} corners`}
					disabled={!selectionAvailable || disabled || unlinkDisabled}
					onpointerdown={(event) => {
						event.preventDefault();
						event.stopPropagation();
					}}
					onclick={(event) => {
						event.stopPropagation();
						unlink(group.corner);
					}}>
					<svg viewBox="0 0 24 24" aria-hidden="true"
						><circle cx="9" cy="9" r="5" /><circle cx="15" cy="15" r="5" /></svg>
				</button>
			{/if}
		{/each}
	</div>
{/each}
{#if ghost}<ViewportHandle
		data-native-overlay-control
		x={ghost.x}
		y={ghost.y}
		label="Warp pointer"
		ghost />{/if}

<style>
	.corner-link {
		position: absolute;
		width: 1.2rem;
		height: 1.2rem;
		transform: translate(-50%, -50%);
		padding: 0.1rem;
		border: 0;
		border-radius: 50%;
		background: var(--gc-background-color, #202734);
		color: var(--gc-focus-color, #a3bfff);
		pointer-events: auto;
		cursor: pointer;
	}
	.corner-link:disabled {
		cursor: default;
	}
	.corner-link svg {
		position: static;
		width: 100%;
		height: 100%;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.5;
	}
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
		cursor: pointer;
	}
</style>
