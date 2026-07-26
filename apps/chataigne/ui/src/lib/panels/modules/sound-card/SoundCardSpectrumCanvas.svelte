<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { SpectrumBandObservation } from 'golden_audio_ui';
	import { CanvasRenderScheduler } from './canvas-render-scheduler';

	let { bands, label = 'Spectrum analysis' } = $props<{
		bands: readonly SpectrumBandObservation[];
		label?: string;
	}>();

	let canvas: HTMLCanvasElement | null = $state(null);
	let cssWidth = $state(0);
	let cssHeight = $state(0);
	let renderReady = $state(false);
	let scheduler: CanvasRenderScheduler | null = null;
	let resizeObserver: ResizeObserver | null = null;

	const draw = (): void => {
		if (!canvas || cssWidth <= 0 || cssHeight <= 0) return;
		const context = canvas.getContext('2d');
		if (!context) return;
		const ratio = Math.max(1, globalThis.devicePixelRatio ?? 1);
		const width = Math.max(1, Math.round(cssWidth * ratio));
		const height = Math.max(1, Math.round(cssHeight * ratio));
		if (canvas.width !== width) canvas.width = width;
		if (canvas.height !== height) canvas.height = height;
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		context.clearRect(0, 0, cssWidth, cssHeight);
		context.fillStyle = '#141a25';
		context.fillRect(0, 0, cssWidth, cssHeight);
		if (bands.length === 0) return;

		const gap = Math.min(0.15 * (cssWidth / bands.length), 0.18 * ratio);
		const barWidth = cssWidth / bands.length;
		context.fillStyle = '#62b5f3';
		for (let index = 0; index < bands.length; index += 1) {
			const normalized = Math.max(
				0,
				Math.min(1, (Math.max(-96, bands[index].amplitude_dbfs) + 96) / 96)
			);
			const barHeight = normalized * cssHeight;
			context.fillRect(
				index * barWidth + gap,
				cssHeight - barHeight,
				Math.max(0.1, barWidth - gap * 2),
				barHeight
			);
		}
	};

	$effect(() => {
		bands;
		cssWidth;
		cssHeight;
		renderReady;
		scheduler?.request(draw);
	});

	onMount(() => {
		if (!canvas) return;
		scheduler = new CanvasRenderScheduler(requestAnimationFrame, cancelAnimationFrame);
		resizeObserver = new ResizeObserver(([entry]) => {
			cssWidth = entry.contentRect.width;
			cssHeight = entry.contentRect.height;
		});
		resizeObserver.observe(canvas);
		renderReady = true;
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		scheduler?.dispose();
	});
</script>

<canvas bind:this={canvas} aria-label={label}>
	{label}. {bands.length} spectrum bands.
</canvas>

<style>
	canvas {
		display: block;
		inline-size: 100%;
		block-size: 9rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: #141a25;
	}
</style>
