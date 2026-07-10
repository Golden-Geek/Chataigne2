<script lang="ts">
	import type { ParamValue, UiParamConstraints } from '../../../types';
	import { MultiSeriesRingBuffer } from './watcher-buffers';
	import {
		extractColorSample,
		getNowMs,
		toCssColor,
		type WatcherDecimationMode,
		type WatcherRangeMode
	} from './watcher-utils';

	let {
		sampleValue,
		constraints: _constraints,
		timeWindowMs,
		rangeMode: _rangeMode,
		unit: _unit,
		streamKey,
		decimationMode: _decimationMode
	} = $props<{
		sampleValue: ParamValue;
		constraints: UiParamConstraints;
		timeWindowMs: number;
		rangeMode: WatcherRangeMode;
		unit: string;
		streamKey: string;
		decimationMode: WatcherDecimationMode;
	}>();

	const FRAME_INTERVAL_MS = 32;

	let canvasElement = $state<HTMLCanvasElement | null>(null);
	let latestColorText = $state('rgba(0 0 0 / 1)');
	let latestColor = $state<[number, number, number, number]>([0, 0, 0, 1]);

	let colorBuffer: MultiSeriesRingBuffer | null = null;
	let lastSignature = '';

	$effect(() => {
		streamKey;
		colorBuffer = null;
		lastSignature = '';
		const resetColor: [number, number, number, number] = [0, 0, 0, 1];
		latestColor = resetColor;
		latestColorText = toCssColor(resetColor);
	});

	$effect(() => {
		const colorSample = extractColorSample(sampleValue);
		if (!colorSample) {
			return;
		}
		if (!colorBuffer) {
			colorBuffer = new MultiSeriesRingBuffer(4, 8192);
			lastSignature = '';
		}

		const signature = colorSample.map((entry) => String(entry)).join('|');
		if (signature === lastSignature) {
			return;
		}

		lastSignature = signature;
		const normalizedColor = [...colorSample] as [number, number, number, number];
		latestColor = normalizedColor;
		latestColorText = toCssColor(normalizedColor);
		colorBuffer.push(getNowMs(), normalizedColor);
	});

	const drawCheckerboard = (
		context: CanvasRenderingContext2D,
		width: number,
		height: number
	): void => {
		const cellSize = Math.max(2, Math.round(height / 7));
		for (let y = 0; y < height; y += cellSize) {
			for (let x = 0; x < width; x += cellSize) {
				const odd = ((x / cellSize) | 0) % 2 !== ((y / cellSize) | 0) % 2;
				context.fillStyle = odd ? 'rgba(255 255 255 / 0.1)' : 'rgba(255 255 255 / 0.04)';
				context.fillRect(x, y, cellSize, cellSize);
			}
		}
	};

	const colorAt = (
		buffer: MultiSeriesRingBuffer,
		sampleIndex: number
	): [number, number, number, number] => [
		buffer.valueAt(0, sampleIndex),
		buffer.valueAt(1, sampleIndex),
		buffer.valueAt(2, sampleIndex),
		buffer.valueAt(3, sampleIndex)
	];

	$effect(() => {
		if (!canvasElement || typeof window === 'undefined') {
			return;
		}
		const context = canvasElement.getContext('2d');
		if (!context) {
			return;
		}

		let disposed = false;
		let animationFrameId = 0;
		let lastFrameAtMs = 0;
		let width = 1;
		let height = 1;

		const syncCanvasSize = (): void => {
			if (!canvasElement) {
				return;
			}
			const rect = canvasElement.getBoundingClientRect();
			const cssWidth = Math.max(1, rect.width);
			const cssHeight = Math.max(1, rect.height);
			const dpr = Math.max(1, window.devicePixelRatio || 1);
			const deviceWidth = Math.max(1, Math.round(cssWidth * dpr));
			const deviceHeight = Math.max(1, Math.round(cssHeight * dpr));

			width = cssWidth;
			height = cssHeight;

			if (canvasElement.width === deviceWidth && canvasElement.height === deviceHeight) {
				return;
			}

			canvasElement.width = deviceWidth;
			canvasElement.height = deviceHeight;
			context.setTransform(dpr, 0, 0, dpr, 0, 0);
		};

		const draw = (nowMs: number): void => {
			syncCanvasSize();
			context.clearRect(0, 0, width, height);
			drawCheckerboard(context, width, height);

			const safeWindowMs = Math.max(250, timeWindowMs);
			const windowStartMs = nowMs - safeWindowMs;

			const buffer = colorBuffer;
			if (!buffer || buffer.length === 0) {
				context.fillStyle = latestColorText;
				context.globalAlpha = 0.6;
				context.fillRect(0, 0, width, height);
				context.globalAlpha = 1;
				context.strokeStyle = 'rgba(255 255 255 / 0.2)';
				context.lineWidth = 1;
				context.strokeRect(0.5, 0.5, Math.max(0, width - 1), Math.max(0, height - 1));
				return;
			}

			let previousTimeMs = windowStartMs;
			const previousIndex = buffer.findLatestIndexAtOrBefore(windowStartMs);
			let previousColor =
				previousIndex !== null
					? colorAt(buffer, previousIndex)
					: (latestColor as [number, number, number, number]);

			buffer.forEachSince(windowStartMs, (sampleIndex, sampleTimeMs) => {
				const nextColor = colorAt(buffer, sampleIndex);
				const x1 = Math.max(0, ((previousTimeMs - windowStartMs) / safeWindowMs) * width);
				const x2 = Math.max(0, ((sampleTimeMs - windowStartMs) / safeWindowMs) * width);
				const segmentWidth = Math.max(0, x2 - x1);

				if (segmentWidth > 0) {
					context.fillStyle = toCssColor(previousColor);
					context.fillRect(x1, 0, segmentWidth, height);
				}

				previousTimeMs = sampleTimeMs;
				previousColor = nextColor;
			});

			const trailingX = Math.max(0, ((previousTimeMs - windowStartMs) / safeWindowMs) * width);
			context.fillStyle = toCssColor(previousColor);
			context.fillRect(trailingX, 0, Math.max(0, width - trailingX), height);

			context.strokeStyle = 'rgba(255 255 255 / 0.2)';
			context.lineWidth = 1;
			context.strokeRect(0.5, 0.5, Math.max(0, width - 1), Math.max(0, height - 1));
		};

		const frame = (nowMs: number): void => {
			if (disposed) {
				return;
			}

			if (nowMs - lastFrameAtMs >= FRAME_INTERVAL_MS) {
				draw(nowMs);
				lastFrameAtMs = nowMs;
			}

			animationFrameId = window.requestAnimationFrame(frame);
		};

		let resizeFrameId: number | null = null;
		const scheduleCanvasSizeSync = (): void => {
			if (typeof window === 'undefined') {
				syncCanvasSize();
				return;
			}
			if (resizeFrameId !== null) {
				return;
			}
			resizeFrameId = window.requestAnimationFrame(() => {
				resizeFrameId = null;
				syncCanvasSize();
			});
		};

		let resizeObserver: ResizeObserver | null = null;
		if (typeof ResizeObserver !== 'undefined') {
			resizeObserver = new ResizeObserver(() => {
				scheduleCanvasSizeSync();
			});
			resizeObserver.observe(canvasElement);
		}

		syncCanvasSize();
		animationFrameId = window.requestAnimationFrame(frame);

		return () => {
			disposed = true;
			if (resizeFrameId !== null && typeof window !== 'undefined') {
				window.cancelAnimationFrame(resizeFrameId);
			}
			window.cancelAnimationFrame(animationFrameId);
			resizeObserver?.disconnect();
		};
	});
</script>

<div class="watcher-color">
	<div class="watcher-color-meta">
		<span class="swatch" style="--watcher-color: {latestColorText}"></span>
		<span class="color-value">{latestColorText}</span>
	</div>
	<canvas bind:this={canvasElement} class="watcher-color-canvas"></canvas>
</div>

<style>
	.watcher-color {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.watcher-color-meta {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.65rem;
	}

	.swatch {
		width: 0.85rem;
		height: 0.85rem;
		border-radius: 0.2rem;
		background-color: var(--watcher-color);
		border: solid 0.06rem rgba(255 255 255 / 0.2);
		flex-shrink: 0;
	}

	.color-value {
		opacity: 0.9;
	}

	.watcher-color-canvas {
		display: block;
		width: 100%;
		height: 6rem;
		border-radius: 0.4rem;
	}
</style>
