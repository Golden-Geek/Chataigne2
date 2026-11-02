<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
	import { cameraZoomFactor } from 'golden_graph_ui/camera-geometry';
	import IconButton from '../common/IconButton.svelte';
	import homeIcon from '../../style/icons/viewport/home.svg';
	import frameIcon from '../../style/icons/viewport/frame.svg';
	import zoomInIcon from '../../style/icons/viewport/zoom-in.svg';
	import zoomOutIcon from '../../style/icons/viewport/zoom-out.svg';
	import { fitSize, frameBounds, zoomAt, type ViewBounds, type ViewCamera } from './geometry';
	let {
		aspect,
		identity,
		selection = null,
		label = '2D editor view',
		native = false,
		children
	}: {
		aspect: number;
		identity: string;
		selection?: ViewBounds | null;
		label?: string;
		/** Native underlays supply black themselves; their DOM aperture must stay transparent. */
		native?: boolean;
		children: Snippet;
	} = $props();
	let element: HTMLDivElement;
	let width = $state(0),
		height = $state(0);
	let camera = $state<ViewCamera>({ zoom: 1, pan: { x: 0, y: 0 } });
	let space = $state(false);
	let appliedIdentity: string | undefined;
	let drag = $state<{ id: number; x: number; y: number; panX: number; panY: number } | null>(null);
	const size = $derived(fitSize(width, height, aspect));
	function home() {
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
		camera = zoomAt(camera, camera.zoom * factor, { x, y });
	}
	function frame() {
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
		if (isControl(event.target)) return;
		element.focus({ preventScroll: true });
		if (drag || !(event.button === 1 || (event.button === 0 && space))) return;
		event.preventDefault();
		event.stopPropagation();
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
		if (!drag || (event && event.pointerId !== drag.id)) return;
		const id = drag.id;
		drag = null;
		if (element.hasPointerCapture(id)) element.releasePointerCapture(id);
	}
	function keydown(event: KeyboardEvent) {
		if (
			event.defaultPrevented ||
			isEditable(event.target) ||
			event.ctrlKey ||
			event.metaKey ||
			event.altKey
		)
			return;
		if (event.code === 'Space') {
			if (event.target instanceof Element && event.target.closest('button, a, [role="button"]'))
				return;
			space = true;
		} else if (event.key === 'Home' || event.key.toLowerCase() === 'h') home();
		else if (event.key.toLowerCase() === 'f') frame();
		else if (event.key === '+' || event.key === '=') zoom(1.2);
		else if (event.key === '-') zoom(1 / 1.2);
		else return;
		event.preventDefault();
		event.stopPropagation();
	}
</script>

<svelte:window
	onkeyup={(event) => {
		if (event.code === 'Space') space = false;
	}}
	onblur={() => {
		space = false;
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
	onpointerdowncapture={down}
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
		if (isControl(event.target) || drag) return;
		event.preventDefault();
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
			style:width={`${size.width * camera.zoom}px`}
			style:height={`${size.height * camera.zoom}px`}
			style:left={`calc(50% + ${camera.pan.x}px)`}
			style:top={`calc(50% + ${camera.pan.y}px)`}>
			{@render children()}
		</div>
	</div>
	<div class="navigation" role="toolbar" aria-label="View navigation">
		<IconButton
			icon={zoomOutIcon}
			label="Zoom out"
			title="Zoom out (-)"
			onclick={() => zoom(1 / 1.2)} />
		<output aria-label="View zoom">{Math.round(camera.zoom * 100)}%</output>
		<IconButton icon={zoomInIcon} label="Zoom in" title="Zoom in (+)" onclick={() => zoom(1.2)} />
		<IconButton
			icon={frameIcon}
			label="Frame selection"
			title="Frame selection (F)"
			disabled={!selection}
			onclick={frame} />
		<IconButton icon={homeIcon} label="Home view" title="Home view (H / Home)" onclick={home} />
	</div>
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
	.panning,
	.panning :global(*) {
		cursor: grab !important;
	}
	.dragging,
	.dragging :global(*) {
		cursor: grabbing !important;
	}
	.navigation {
		position: absolute;
		inset: 0.35rem 0.35rem auto auto;
		display: flex;
		align-items: center;
		gap: 0.1rem;
		padding: 0.15rem;
		border-radius: 0.3rem;
		background: var(--gc-color-panel, #252525);
		color: var(--gc-color-text, #ddd);
	}
	output {
		min-width: 3em;
		text-align: center;
		font-size: 0.75rem;
		font-variant-numeric: tabular-nums;
	}
</style>
