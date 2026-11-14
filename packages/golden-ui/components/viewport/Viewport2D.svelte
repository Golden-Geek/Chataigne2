<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { cameraZoomFactor } from 'golden_graph_ui/camera-geometry';
	import { CanvasToolbar, canvasNavigationCommand } from 'golden_graph_ui';
	import { fitSize, frameBounds, type ViewBounds, type ViewCamera } from './geometry';
	import { createZoomMotion } from './zoom-motion';
	let {
		aspect,
		identity,
		selection = null,
		label = '2D editor view',
		native = false,
		nativeFrame = false,
		onselectionclear,
		onkeydown: onEditorKeyDown,
		onfocusout,
		children,
		actions
	}: {
		aspect: number;
		identity: string;
		selection?: ViewBounds | null;
		label?: string;
		/** Native underlays supply content black themselves; the DOM aperture stays transparent. */
		native?: boolean;
		/** An installed underlay owns the image frame and surrounding canvas paint. */
		nativeFrame?: boolean;
		/** Unconsumed background primary-down or idle Escape; adapters resolve their edited owner. */
		onselectionclear?(event: PointerEvent | KeyboardEvent): void;
		onkeydown?(event: KeyboardEvent): void;
		onfocusout?(event: FocusEvent): void;
		children: Snippet;
		actions?: Snippet;
	} = $props();
	let element: HTMLDivElement;
	let width = $state(0),
		height = $state(0);
	let camera = $state<ViewCamera>({ zoom: 1, pan: { x: 0, y: 0 } });
	let space = $state(false);
	let pressed = $state<number | null>(null);
	let suppressClick = false;
	const motion = createZoomMotion(
		() => camera,
		(next) => {
			camera = next;
		}
	);
	let appliedIdentity: string | undefined;
	let appliedSize = { width: 0, height: 0 };
	let drag = $state<{ id: number; x: number; y: number; panX: number; panY: number } | null>(null);
	const size = $derived(fitSize(width, height, aspect));
	function home() {
		motion.cancel();
		release();
		camera = { zoom: 1, pan: { x: 0, y: 0 } };
	}
	$effect(() => {
		const current = identity;
		// Parent snapshot replacement invalidates a prop getter even when its ID is unchanged.
		// Geometry receipts and selection updates must never reframe an ongoing interaction.
		if (current === appliedIdentity) return;
		appliedIdentity = current;
		untrack(home);
	});
	function zoom(factor: number, x = 0, y = 0) {
		if (pressed !== null) return;
		motion.zoom(factor, { x, y }, window.matchMedia('(prefers-reduced-motion: reduce)').matches);
	}
	function frame() {
		motion.cancel();
		release();
		if (!selection) return home();
		const rem = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
		const next = frameBounds(selection, size, { width, height }, 3 * rem);
		if (next) camera = next;
	}
	function isEditable(target: EventTarget | null) {
		return (
			target instanceof Element &&
			!!target.closest('input, textarea, select, [contenteditable="true"]')
		);
	}
	function isControl(target: EventTarget | null) {
		return isEditable(target) || (target instanceof Element && !!target.closest('.navigation'));
	}
	function down(event: PointerEvent) {
		suppressClick = false;
		if (isControl(event.target)) return;
		motion.cancel();
		pressed ??= event.pointerId;
		element.focus({ preventScroll: true });
		if (drag || !(event.button === 1 || (event.button === 0 && space))) return;
		event.preventDefault();
		event.stopPropagation();
		suppressClick = true;
		drag = {
			id: event.pointerId,
			x: event.clientX,
			y: event.clientY,
			panX: camera.pan.x,
			panY: camera.pan.y
		};
		element.setPointerCapture(event.pointerId);
	}
	function release(event?: PointerEvent) {
		if (!event || event.pointerId === pressed) pressed = null;
		if (!drag || (event && event.pointerId !== drag.id)) return;
		const id = drag.id;
		drag = null;
		if (element.hasPointerCapture(id)) element.releasePointerCapture(id);
	}
	function keydown(event: KeyboardEvent) {
		if (event.defaultPrevented || isEditable(event.target)) return;
		if (!isControl(event.target)) onEditorKeyDown?.(event);
		if (
			event.defaultPrevented ||
			pressed !== null ||
			isEditable(event.target) ||
			event.ctrlKey ||
			event.metaKey ||
			event.altKey
		)
			return;
		if (event.key === 'Escape' && !isControl(event.target) && onselectionclear) {
			event.preventDefault();
			event.stopPropagation();
			onselectionclear(event);
			return;
		}
		if (event.code === 'Space') {
			if (event.target instanceof Element && event.target.closest('button, a, [role="button"]'))
				return;
			space = true;
		} else {
			const command = canvasNavigationCommand(event);
			if (command === 'home') home();
			else if (command === 'frame') frame();
			else if (command === 'zoom-in') zoom(1.2);
			else if (command === 'zoom-out') zoom(1 / 1.2);
			else return;
		}
		event.preventDefault();
		event.stopPropagation();
	}
	$effect(() => {
		const next = size;
		if (next.width === appliedSize.width && next.height === appliedSize.height) return;
		appliedSize = next;
		untrack(motion.cancel);
	});
	onDestroy(() => {
		motion.dispose();
		release();
	});
</script>

<svelte:window
	onpointerup={release}
	onpointercancel={release}
	onkeyup={(event) => {
		if (event.code === 'Space') space = false;
	}}
	onblur={() => {
		space = false;
		motion.cancel();
		release();
	}} />

<!-- Canvas keyboard navigation needs a focusable application surface with nested tools. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
	bind:this={element}
	class="viewport-2d"
	data-native-preview-backdrop={native ? '' : undefined}
	class:panning={space || drag !== null}
	class:dragging={drag !== null}
	role="application"
	aria-label={label}
	tabindex="0"
	onkeydown={keydown}
	{onfocusout}
	onpointerdowncapture={down}
	onclickcapture={(event) => {
		if (!suppressClick) return;
		suppressClick = false;
		if (event.detail === 0) return;
		event.preventDefault();
		event.stopPropagation();
	}}
	onpointerdown={(event) => {
		if (
			!event.defaultPrevented &&
			event.button === 0 &&
			!isControl(event.target) &&
			!(event.target instanceof Element && event.target.closest('button, a, [role="button"]'))
		)
			onselectionclear?.(event);
	}}
	onpointermove={(event) => {
		if (drag?.id === event.pointerId) {
			camera = {
				...camera,
				pan: { x: drag.panX + event.clientX - drag.x, y: drag.panY + event.clientY - drag.y }
			};
			event.stopPropagation();
		}
	}}
	onpointerup={release}
	onpointercancel={release}
	onlostpointercapture={release}
	oncontextmenu={(event) => {
		if (space || drag) event.preventDefault();
	}}
	onwheel={(event) => {
		if (isControl(event.target)) return;
		event.preventDefault();
		if (pressed !== null) return;
		element.focus({ preventScroll: true });
		const rect = element.getBoundingClientRect();
		zoom(
			cameraZoomFactor(event.deltaY, event.deltaMode, height),
			event.clientX - rect.left - rect.width / 2,
			event.clientY - rect.top - rect.height / 2
		);
	}}>
	<div class="stage" bind:clientWidth={width} bind:clientHeight={height}>
		<div
			class="active-zone"
			class:native
			class:native-frame={native && nativeFrame}
			style:width={`${size.width * camera.zoom}px`}
			style:height={`${size.height * camera.zoom}px`}
			style:left={`calc(50% + ${camera.pan.x}px)`}
			style:top={`calc(50% + ${camera.pan.y}px)`}>
			{@render children()}
		</div>
	</div>
	<CanvasToolbar
		zoom={camera.zoom}
		minZoom={0.1}
		maxZoom={16}
		{frame}
		{home}
		canFrame={!!selection}
		setZoom={(next) => zoom(next / camera.zoom)}
		{actions} />
</div>

<style>
	.viewport-2d {
		position: relative;
		width: 100%;
		height: 100%;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		touch-action: none;
		outline: none;
	}
	.viewport-2d:focus-visible {
		outline: 0.1rem solid var(--gc-focus-color, #a3bfff);
		outline-offset: -0.1rem;
	}
	.stage {
		position: absolute;
		inset: 1rem;
		pointer-events: none;
	}
	.active-zone {
		position: absolute;
		transform: translate(-50%, -50%);
		background: #000;
		/* Paint the themed surround without putting an opaque ancestor over native pixels. */
		box-shadow: 0 0 0 100vmax var(--gc-color-panel, #252525);
		outline: 0.0625rem solid var(--gc-panel-border-color, #666);
		pointer-events: auto;
		--preview-unavailable-background: #000;
	}
	.active-zone.native {
		background: transparent;
	}
	.active-zone.native-frame {
		/* A moving DOM surround would clip an independently presented native frame,
		   including unrestricted handles outside the image. The underlay paints both. */
		box-shadow: none;
		outline: none;
	}
	.panning,
	.panning :global(*) {
		cursor: grab !important;
	}
	.dragging,
	.dragging :global(*) {
		cursor: grabbing !important;
	}
</style>
