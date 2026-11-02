<script lang="ts">
	import { untrack, type Snippet } from 'svelte';
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
	let space = $state(false),
		panTool = $state(false);
	let drag = $state<{ id: number; x: number; y: number; panX: number; panY: number } | null>(null);
	const size = $derived(fitSize(width, height, aspect));
	function home() {
		camera = { zoom: 1, pan: { x: 0, y: 0 } };
	}
	$effect(() => {
		identity;
		untrack(home);
	});
	function zoom(factor: number, x = 0, y = 0) {
		camera = zoomAt(camera, camera.zoom * factor, { x, y });
	}
	function frame() {
		if (!selection) return;
		const next = frameBounds(selection, aspect);
		if (next)
			camera = {
				zoom: next.zoom,
				pan: { x: next.pan.x * size.width, y: next.pan.y * size.height }
			};
	}
	function isControl(target: EventTarget | null) {
		return (
			target instanceof Element &&
			!!target.closest('input, textarea, select, [contenteditable="true"], .navigation')
		);
	}
	function down(event: PointerEvent) {
		if (isControl(event.target)) return;
		element.focus({ preventScroll: true });
		if (drag || !(event.button === 1 || (event.button === 0 && (space || panTool)))) return;
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
		if (isControl(event.target) || event.ctrlKey || event.metaKey || event.altKey) return;
		if (event.code === 'Space') space = true;
		else if (event.key === 'Home') home();
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
	class:panning={space || panTool || drag !== null}
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
		if (space || panTool || drag) event.preventDefault();
	}}
	onwheel={(event) => {
		if (isControl(event.target) || drag) return;
		event.preventDefault();
		element.focus({ preventScroll: true });
		const rect = element.getBoundingClientRect();
		const delta = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? height : 1);
		zoom(
			Math.exp(-Math.max(-500, Math.min(500, delta)) * 0.002),
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
		<button
			type="button"
			aria-label="Pan view"
			aria-pressed={panTool}
			title="Pan view (Space + drag or middle mouse)"
			onclick={() => {
				panTool = !panTool;
				element.focus();
			}}>
			<svg viewBox="0 0 24 24" aria-hidden="true"
				><path d="M12 3v18M3 12h18M9 6l3-3 3 3M9 18l3 3 3-3M6 9l-3 3 3 3M18 9l3 3-3 3" /></svg>
		</button>
		<button type="button" aria-label="Zoom out" title="Zoom out (-)" onclick={() => zoom(1 / 1.2)}
			><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14" /></svg></button>
		<output aria-label="View zoom">{Math.round(camera.zoom * 100)}%</output>
		<button type="button" aria-label="Zoom in" title="Zoom in (+)" onclick={() => zoom(1.2)}
			><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14M12 5v14" /></svg></button>
		<button
			type="button"
			aria-label="Frame selection"
			title="Frame selection (F)"
			disabled={!selection}
			onclick={frame}
			><svg viewBox="0 0 24 24" aria-hidden="true"
				><path d="M9 3H3v6M15 3h6v6M3 15v6h6M21 15v6h-6M8 8h8v8H8z" /></svg
			></button>
		<button type="button" aria-label="Home view" title="Fit active area (Home)" onclick={home}
			><svg viewBox="0 0 24 24" aria-hidden="true"
				><path d="m3 11 9-8 9 8M6 9v12h12V9M10 21v-7h4v7" /></svg
			></button>
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
	.navigation {
		position: absolute;
		inset: 0.35rem 0.35rem auto auto;
		display: flex;
		align-items: center;
		gap: 0.1rem;
		padding: 0.15rem;
		border-radius: 0.3rem;
		background: var(--gc-dock-panel-background, #252525);
		color: var(--text-color, #ddd);
	}
	button {
		display: grid;
		place-items: center;
		width: 1.65rem;
		height: 1.65rem;
		border: 0;
		padding: 0.2rem;
		border-radius: 0.2rem;
		background: transparent;
		color: inherit;
		cursor: pointer;
	}
	button:hover,
	button[aria-pressed='true'] {
		background: var(--gc-control-hover-background, #ffffff18);
	}
	button:disabled {
		opacity: 0.35;
		cursor: default;
	}
	button:focus-visible {
		outline: 0.1rem solid var(--gc-focus-color, #a3bfff);
	}
	.navigation svg {
		width: 100%;
		height: 100%;
		fill: none;
		stroke: currentColor;
		stroke-width: 1.5;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	output {
		min-width: 3em;
		text-align: center;
		font-size: 0.75rem;
		font-variant-numeric: tabular-nums;
	}
</style>
