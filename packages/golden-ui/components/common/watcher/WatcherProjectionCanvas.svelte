<script lang="ts">
	import { formatWatcherNumber } from './watcher-utils';
	import type { MultiSeriesRingBuffer } from './watcher-buffers';

	let {
		buffer = null,
		xChannel,
		yChannel,
		timeWindowMs,
		rangeMin,
		rangeMax,
		color,
		latestPoint,
		axisLabel = ''
	} = $props<{
		buffer: MultiSeriesRingBuffer | null;
		xChannel: number;
		yChannel: number;
		timeWindowMs: number;
		rangeMin: number;
		rangeMax: number;
		color: string;
		latestPoint: [number, number];
		axisLabel?: string;
	}>();

	const FRAME_INTERVAL_MS = 32;
	const EPSILON = 1e-6;

	const computeNiceStep = (span: number, targetTickCount = 8): number => {
		const safeSpan = Math.max(EPSILON, Math.abs(span));
		const rough = safeSpan / Math.max(2, targetTickCount);
		const power = Math.floor(Math.log10(rough));
		const magnitude = Math.pow(10, power);
		const normalized = rough / magnitude;

		let nice = 1;
		if (normalized >= 5) {
			nice = 5;
		} else if (normalized >= 2) {
			nice = 2;
		} else {
			nice = 1;
		}
		return nice * magnitude;
	};

	const approxZero = (value: number, epsilon: number): boolean => Math.abs(value) <= epsilon;

	let canvasElement = $state<HTMLCanvasElement | null>(null);

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
			const safeWindowMs = Math.max(250, timeWindowMs);
			const windowStartMs = nowMs - safeWindowMs;
			const gapBreakMs = Math.max(120, safeWindowMs * 0.15);

			const minValue = Number.isFinite(rangeMin) ? rangeMin : -1;
			const maxValue = Number.isFinite(rangeMax) ? rangeMax : 1;
			const span = Math.max(EPSILON, maxValue - minValue);

			const padX = width * 0.08;
			const padY = height * 0.1;
			const availableW = Math.max(1, width - padX * 2);
			const availableH = Math.max(1, height - padY * 2);
			const plotSize = Math.max(1, Math.min(availableW, availableH));
			const plotLeft = padX + (availableW - plotSize) * 0.5;
			const plotTop = padY + (availableH - plotSize) * 0.5;

			const mapX = (value: number): number => plotLeft + ((value - minValue) / span) * plotSize;
			const mapY = (value: number): number =>
				plotTop + plotSize - ((value - minValue) / span) * plotSize;

			context.clearRect(0, 0, width, height);
			context.fillStyle = 'rgba(255 255 255 / 0.02)';
			context.fillRect(0, 0, width, height);

			const tickStep = computeNiceStep(span, 8);
			const firstTick = Math.ceil(minValue / tickStep) * tickStep;
			const epsilonTick = tickStep * 1e-6;
			const labelFontSize = Math.max(8, Math.round(height * 0.062));

			let tickCount = 0;
			for (
				let tickValue = firstTick;
				tickValue <= maxValue + epsilonTick && tickCount < 128;
				tickValue += tickStep, tickCount += 1
			) {
				const x = mapX(tickValue);
				const y = mapY(tickValue);
				const tickOrdinal = Math.round(tickValue / tickStep);
				const isMajor = tickOrdinal % 5 === 0;
				const isZero = approxZero(tickValue, tickStep * 0.25);

				context.strokeStyle = isZero
					? 'rgba(255 255 255 / 0.24)'
					: isMajor
						? 'rgba(255 255 255 / 0.14)'
						: 'rgba(255 255 255 / 0.08)';
				context.lineWidth = isZero ? 1.1 : isMajor ? 0.95 : 0.7;
				context.beginPath();
				context.moveTo(x, plotTop);
				context.lineTo(x, plotTop + plotSize);
				context.moveTo(plotLeft, y);
				context.lineTo(plotLeft + plotSize, y);
				context.stroke();

				if (isMajor && !isZero) {
					context.fillStyle = 'rgba(255 255 255 / 0.42)';
					context.font = `${labelFontSize}px sans-serif`;
					context.fillText(formatWatcherNumber(tickValue), x + 2, plotTop + plotSize - 2);
				}
			}

			if (minValue < 0 && maxValue > 0) {
				const zeroX = mapX(0);
				const zeroY = mapY(0);
				context.strokeStyle = 'rgba(255 255 255 / 0.2)';
				context.beginPath();
				context.moveTo(zeroX, plotTop);
				context.lineTo(zeroX, plotTop + plotSize);
				context.moveTo(plotLeft, zeroY);
				context.lineTo(plotLeft + plotSize, zeroY);
				context.stroke();
			}

			const drawTrailSegment = (
				startTimeMs: number,
				endTimeMs: number,
				startX: number,
				startY: number,
				endX: number,
				endY: number
			): void => {
				const ageAtSegment = nowMs - (startTimeMs + endTimeMs) * 0.5;
				const ageNorm = Math.max(0, Math.min(1, ageAtSegment / safeWindowMs));
				const intensity = 1 - ageNorm;

				context.globalAlpha = 0.06 + intensity * 0.9;
				context.lineWidth = 0.7 + intensity * 1.1;
				context.strokeStyle = color;
				context.beginPath();
				context.moveTo(mapX(startX), mapY(startY));
				context.lineTo(mapX(endX), mapY(endY));
				context.stroke();
			};

			if (buffer && buffer.length > 0) {
				let previousTimeMs = windowStartMs;
				let previousX = latestPoint[0];
				let previousY = latestPoint[1];
				let hasPrevious = false;

				const beforeIndex = buffer.findLatestIndexAtOrBefore(windowStartMs);
				if (beforeIndex !== null) {
					previousX = buffer.valueAt(xChannel, beforeIndex);
					previousY = buffer.valueAt(yChannel, beforeIndex);
					hasPrevious = true;
				}

				buffer.forEachSince(windowStartMs, (sampleIndex: number, sampleTimeMs: number) => {
					const nextX = buffer?.valueAt(xChannel, sampleIndex) ?? 0;
					const nextY = buffer?.valueAt(yChannel, sampleIndex) ?? 0;

					if (!hasPrevious) {
						previousTimeMs = sampleTimeMs;
						previousX = nextX;
						previousY = nextY;
						hasPrevious = true;
						return;
					}

					if (sampleTimeMs - previousTimeMs <= gapBreakMs) {
						drawTrailSegment(previousTimeMs, sampleTimeMs, previousX, previousY, nextX, nextY);
					}

					previousTimeMs = sampleTimeMs;
					previousX = nextX;
					previousY = nextY;
				});
			}

			context.globalAlpha = 0.3;
			context.fillStyle = color;
			context.beginPath();
			context.arc(
				mapX(latestPoint[0]),
				mapY(latestPoint[1]),
				Math.max(2.2, Math.min(width, height) * 0.022),
				0,
				Math.PI * 2
			);
			context.fill();

			context.globalAlpha = 1;
			context.beginPath();
			context.arc(
				mapX(latestPoint[0]),
				mapY(latestPoint[1]),
				Math.max(1.2, Math.min(width, height) * 0.012),
				0,
				Math.PI * 2
			);
			context.fill();

			context.strokeStyle = 'rgba(255 255 255 / 0.2)';
			context.lineWidth = 1;
			context.strokeRect(plotLeft, plotTop, plotSize, plotSize);

			if (axisLabel.length > 0) {
				context.fillStyle = 'rgba(255 255 255 / 0.7)';
				context.font = `${Math.max(9, Math.round(height * 0.065))}px sans-serif`;
				context.fillText(axisLabel, plotLeft + plotSize * 0.05, plotTop + plotSize * 0.14);
			}
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

<canvas bind:this={canvasElement} class="watcher-projection-canvas"></canvas>

<style>
	.watcher-projection-canvas {
		display: block;
		width: 100%;
		height: 100%;
		border-radius: 0.4rem;
	}
</style>
