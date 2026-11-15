<script lang="ts">
	import type { CanvasTransform } from './curve/types';
	import {
		initialCurveCanvasTheme,
		readCurveCanvasTheme,
		type CurveCanvasTheme
	} from './curve/canvas-theme';
	import { paintCurveCanvas, type CurveCanvasFrame } from './curve/canvas-paint';

	type CanvasInput = Omit<CurveCanvasFrame, 'width' | 'height' | 'theme' | 'rem'>;
	type RequiredInput = Pick<
		CanvasInput,
		| 'parsedKeys'
		| 'compiledCurve'
		| 'selectedKeyId'
		| 'hoverKeyId'
		| 'hoverCurvePosition'
		| 'sampleCurveAt'
	>;
	type Props = RequiredInput &
		Partial<Omit<CanvasInput, keyof RequiredInput>> & {
			interactionTransform?: CanvasTransform | null;
			canvasElement?: HTMLCanvasElement | null;
			activeCanvasPan?: boolean;
			showBounds?: boolean;
			canvasHeight?: string;
			onpointerdown?: (event: PointerEvent) => void;
			onpointermove?: (event: PointerEvent) => void;
			onpointerup?: (event: PointerEvent) => void;
			onpointercancel?: (event: PointerEvent) => void;
			onpointerleave?: (event: PointerEvent) => void;
			onwheel?: (event: WheelEvent) => void;
			oncontextmenu?: (event: MouseEvent) => void;
			onfocus?: (event: FocusEvent) => void;
			onblur?: (event: FocusEvent) => void;
		};

	let {
		parsedKeys,
		compiledCurve,
		selectedKeyId,
		selectedKeyIds = [],
		selectedCurveOwnerKeyIds = [],
		hoverKeyId,
		hoverCurveOwnerKeyId = null,
		bezierHandles = [],
		activeBezierHandle = null,
		hoverBezierHandle = null,
		hoverCurvePosition,
		drawPathPoints = [],
		selectionRect = null,
		activeCurveRangeConstraint = null,
		fixedViewBounds = null,
		interactionTransform = $bindable<CanvasTransform | null>(null),
		canvasElement = $bindable<HTMLCanvasElement | null>(null),
		activeCanvasPan = false,
		showGrid = true,
		showNumbers = true,
		showBounds = true,
		canvasHeight = 'min(26rem, 38vh)',
		sampleCurveAt,
		onpointerdown,
		onpointermove,
		onpointerup,
		onpointercancel,
		onpointerleave,
		onwheel,
		oncontextmenu,
		onfocus,
		onblur
	}: Props = $props();

	let rem_base_px = $state(16);
	let canvas_context = $state<CanvasRenderingContext2D | null>(null);
	let canvas_width_px = $state(1);
	let canvas_height_px = $state(1);
	let bounds_label = $state('Bounds : X [min: -, max: -] | Y [min: -, max: -]');
	let curve_canvas_theme = $state<CurveCanvasTheme>({ ...initialCurveCanvasTheme });

	const draw_curve_canvas = (): void => {
		if (!canvas_context) return;
		const result = paintCurveCanvas(canvas_context, {
			width: canvas_width_px,
			height: canvas_height_px,
			rem: rem_base_px,
			theme: curve_canvas_theme,
			parsedKeys,
			compiledCurve,
			selectedKeyId,
			selectedKeyIds,
			selectedCurveOwnerKeyIds,
			hoverKeyId,
			hoverCurveOwnerKeyId,
			bezierHandles,
			activeBezierHandle,
			hoverBezierHandle,
			hoverCurvePosition,
			drawPathPoints,
			selectionRect,
			activeCurveRangeConstraint,
			fixedViewBounds,
			showGrid,
			showNumbers,
			sampleCurveAt
		});
		if (bounds_label !== result.boundsLabel) bounds_label = result.boundsLabel;
		interactionTransform = result.transform;
	};

	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}
		const refresh = (): void => {
			const parsed = Number.parseFloat(window.getComputedStyle(document.documentElement).fontSize);
			if (Number.isFinite(parsed) && parsed > 0) {
				rem_base_px = parsed;
			}
		};
		refresh();
		window.addEventListener('resize', refresh);
		return () => {
			window.removeEventListener('resize', refresh);
		};
	});

	$effect(() => {
		if (typeof window === 'undefined') {
			return;
		}
		const refresh_curve_theme = (): void => {
			const styles = window.getComputedStyle(document.documentElement);
			curve_canvas_theme = readCurveCanvasTheme(styles);
		};
		refresh_curve_theme();

		if (typeof MutationObserver === 'undefined') {
			return;
		}
		const observer = new MutationObserver(refresh_curve_theme);
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style']
		});
		return () => {
			observer.disconnect();
		};
	});

	$effect(() => {
		if (!canvasElement || typeof window === 'undefined') {
			canvas_context = null;
			return;
		}
		const context = canvasElement.getContext('2d');
		if (!context) {
			canvas_context = null;
			return;
		}
		canvas_context = context;

		const sync_canvas_size = (): void => {
			if (!canvasElement) {
				return;
			}
			const rect = canvasElement.getBoundingClientRect();
			const css_width = Math.max(1, rect.width);
			const css_height = Math.max(1, rect.height);
			const dpr = Math.max(1, window.devicePixelRatio || 1);
			const device_width = Math.max(1, Math.round(css_width * dpr));
			const device_height = Math.max(1, Math.round(css_height * dpr));

			canvas_width_px = css_width;
			canvas_height_px = css_height;

			if (canvasElement.width !== device_width || canvasElement.height !== device_height) {
				canvasElement.width = device_width;
				canvasElement.height = device_height;
			}
			context.setTransform(dpr, 0, 0, dpr, 0, 0);
		};

		let resizeFrameId: number | null = null;
		const scheduleCanvasSizeSync = (): void => {
			if (typeof window === 'undefined') {
				sync_canvas_size();
				return;
			}
			if (resizeFrameId !== null) {
				return;
			}
			resizeFrameId = window.requestAnimationFrame(() => {
				resizeFrameId = null;
				sync_canvas_size();
			});
		};

		let observer: ResizeObserver | null = null;
		if (typeof ResizeObserver !== 'undefined') {
			observer = new ResizeObserver(() => {
				scheduleCanvasSizeSync();
			});
			observer.observe(canvasElement);
		}
		sync_canvas_size();

		return () => {
			if (resizeFrameId !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(resizeFrameId);
			}
			observer?.disconnect();
			canvas_context = null;
		};
	});

	$effect(() => {
		canvas_context;
		canvas_width_px;
		canvas_height_px;
		compiledCurve;
		parsedKeys;
		selectedKeyId;
		selectedKeyIds;
		selectedCurveOwnerKeyIds;
		hoverKeyId;
		hoverCurveOwnerKeyId;
		bezierHandles;
		activeBezierHandle;
		hoverBezierHandle;
		hoverCurvePosition;
		drawPathPoints;
		selectionRect;
		rem_base_px;
		curve_canvas_theme;
		fixedViewBounds;
		activeCurveRangeConstraint;
		showGrid;
		showNumbers;
		showBounds;
		draw_curve_canvas();
	});
</script>

<div class="curve-canvas-container" style:height={canvasHeight}>
	<div class="curve-canvas-wrap">
		<canvas
			class="curve-canvas {activeCanvasPan ? 'panning' : ''}"
			bind:this={canvasElement}
			data-local-context-menu
			tabindex="0"
			{onpointerdown}
			{onpointermove}
			{onpointerup}
			{onpointercancel}
			{onpointerleave}
			{onwheel}
			{oncontextmenu}
			{onfocus}
			{onblur}></canvas>
	</div>
	{#if showBounds}
		<div class="curve-bounds-readout">{bounds_label}</div>
	{/if}
</div>

<style>
	.curve-canvas-container {
		height: min(26rem, 38vh);
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 100%;
		min-width: 0;
		min-height: 0;
		gap: 0.18rem;
	}

	.curve-canvas-wrap {
		flex: 1 1 auto;
		width: 100%;
		max-width: 100%;
		min-width: 0;
		min-height: 0;
		overflow: hidden;
		background: var(--gc-color-curve-canvas-wrap-bg);
		border: solid 0.06rem var(--gc-color-curve-canvas-wrap-border);
		border-radius: 0.3rem;
	}

	.curve-canvas {
		display: block;
		width: 100%;
		max-width: 100%;
		height: 100%;
		cursor: crosshair;
		background: var(--gc-color-curve-canvas-bg);
	}

	.curve-canvas-wrap:focus,
	.curve-canvas:focus,
	.curve-canvas-wrap:focus-visible,
	.curve-canvas:focus-visible {
		outline: none;
	}

	.curve-canvas.panning {
		cursor: grabbing;
	}

	.curve-bounds-readout {
		font-size: 0.57rem;
		line-height: 1.25;
		padding: 0 0.16rem 0.1rem;
		color: var(--gc-color-curve-bounds-readout);
		opacity: 0.92;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	@media (max-width: 58rem) {
		.curve-canvas-container {
			height: min(22rem, 42vh);
		}
	}
</style>
