<script lang="ts">
	import type { ParamValue, UiParamConstraints } from '../../../types';
	import { MultiSeriesRingBuffer } from './watcher-buffers';
	import {
		extractNumericSample,
		formatWatcherNumber,
		getFixedRange,
		getNowMs,
		type WatcherDecimationMode,
		type WatcherRangeMode,
		watcherSeriesColor
	} from './watcher-utils';

	let { sampleValue, constraints, timeWindowMs, rangeMode, unit, streamKey, decimationMode } =
		$props<{
			sampleValue: ParamValue;
			constraints: UiParamConstraints;
			timeWindowMs: number;
			rangeMode: WatcherRangeMode;
			unit: string;
			streamKey: string;
			decimationMode: WatcherDecimationMode;
		}>();

	const FRAME_INTERVAL_MS = 32;
	const EPSILON = 1e-6;
	const RANGE_SYNC_INTERVAL_MS = 120;

	let canvasElement = $state<HTMLCanvasElement | null>(null);
	let seriesLabels = $state<string[]>([]);
	let latestValues = $state<number[]>([]);
	let rangeMinLabel = $state('0');
	let rangeMaxLabel = $state('1');

	let buffer: MultiSeriesRingBuffer | null = null;
	let lastSignature = '';
	let lastRangeSyncAtMs = 0;

	let unitSuffix = $derived(unit.trim().length > 0 ? ` ${unit.trim()}` : '');

	$effect(() => {
		streamKey;
		buffer = null;
		lastSignature = '';
		seriesLabels = [];
		latestValues = [];
		rangeMinLabel = '0';
		rangeMaxLabel = '1';
	});

	$effect(() => {
		timeWindowMs;
		rangeMode;
		constraints.range;
		decimationMode;
	});

	$effect(() => {
		const numericSample = extractNumericSample(sampleValue);
		if (!numericSample) {
			return;
		}

		if (!buffer || buffer.seriesCount !== numericSample.values.length) {
			buffer = new MultiSeriesRingBuffer(numericSample.values.length, 16384);
			seriesLabels = [...numericSample.labels];
			lastSignature = '';
		}

		const signature = numericSample.values.map((entry) => String(entry)).join('|');
		if (signature === lastSignature) {
			return;
		}

		lastSignature = signature;
		latestValues = [...numericSample.values];
		buffer.push(getNowMs(), numericSample.values);
	});

	const drawEmptyState = (
		context: CanvasRenderingContext2D,
		width: number,
		height: number
	): void => {
		context.clearRect(0, 0, width, height);
		context.fillStyle = 'rgba(255 255 255 / 0.03)';
		context.fillRect(0, 0, width, height);
		context.strokeStyle = 'rgba(255 255 255 / 0.12)';
		context.lineWidth = 1;
		context.strokeRect(0.5, 0.5, Math.max(0, width - 1), Math.max(0, height - 1));
	};

	const drawGrid = (context: CanvasRenderingContext2D, width: number, height: number): void => {
		context.strokeStyle = 'rgba(255 255 255 / 0.08)';
		context.lineWidth = 1;
		context.beginPath();
		for (let lineIndex = 1; lineIndex < 4; lineIndex += 1) {
			const y = (height / 4) * lineIndex;
			context.moveTo(0, y);
			context.lineTo(width, y);
		}
		context.stroke();
	};

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

			context.clearRect(0, 0, width, height);
			context.fillStyle = 'rgba(255 255 255 / 0.02)';
			context.fillRect(0, 0, width, height);
			drawGrid(context, width, height);

			if (!buffer || buffer.length === 0) {
				drawEmptyState(context, width, height);
				return;
			}

			let minValue = Number.POSITIVE_INFINITY;
			let maxValue = Number.NEGATIVE_INFINITY;
			const fixedRange = rangeMode === 'fixed' ? getFixedRange(constraints) : null;
			if (fixedRange) {
				minValue = fixedRange.min;
				maxValue = fixedRange.max;
			} else {
				const adaptiveRange = buffer.minMaxSince(windowStartMs);
				if (adaptiveRange) {
					minValue = adaptiveRange.min;
					maxValue = adaptiveRange.max;
				} else if (latestValues.length > 0) {
					minValue = Math.min(...latestValues);
					maxValue = Math.max(...latestValues);
				}
			}

			if (!Number.isFinite(minValue) || !Number.isFinite(maxValue)) {
				minValue = 0;
				maxValue = 1;
			}

			if (Math.abs(maxValue - minValue) <= EPSILON) {
				const center = maxValue;
				const padding = Math.max(Math.abs(center) * 0.1, 0.5);
				minValue = center - padding;
				maxValue = center + padding;
			}

			const valueSpan = maxValue - minValue;
			const valueToY = (value: number): number => {
				const normalized = Math.max(0, Math.min(1, (value - minValue) / valueSpan));
				return height - normalized * height;
			};
			const latestIndexBeforeWindow = buffer.findLatestIndexAtOrBefore(windowStartMs);
			const visibleSamples = buffer.countSince(windowStartMs);
			const shouldDecimate =
				decimationMode === 'minmax' ||
				(decimationMode === 'auto' && visibleSamples > Math.max(64, Math.floor(width) * 4));

			for (let channelIndex = 0; channelIndex < buffer.seriesCount; channelIndex += 1) {
				if (shouldDecimate) {
					const bucketCount = Math.max(1, Math.floor(width));
					const bucketMin = new Float64Array(bucketCount);
					const bucketMax = new Float64Array(bucketCount);
					const bucketHasSample = new Uint8Array(bucketCount);
					for (let bucketIndex = 0; bucketIndex < bucketCount; bucketIndex += 1) {
						bucketMin[bucketIndex] = Number.POSITIVE_INFINITY;
						bucketMax[bucketIndex] = Number.NEGATIVE_INFINITY;
					}

					const updateBucket = (bucketIndex: number, sampleValue: number): void => {
						if (bucketIndex < 0 || bucketIndex >= bucketCount) {
							return;
						}
						bucketHasSample[bucketIndex] = 1;
						bucketMin[bucketIndex] = Math.min(bucketMin[bucketIndex] ?? sampleValue, sampleValue);
						bucketMax[bucketIndex] = Math.max(bucketMax[bucketIndex] ?? sampleValue, sampleValue);
					};

					if (latestIndexBeforeWindow !== null) {
						updateBucket(0, buffer.valueAt(channelIndex, latestIndexBeforeWindow));
					}

					buffer.forEachSince(windowStartMs, (sampleIndex, sampleTimeMs) => {
						const x = ((sampleTimeMs - windowStartMs) / safeWindowMs) * width;
						const bucketIndex = Math.max(
							0,
							Math.min(bucketCount - 1, Math.floor((x / Math.max(1, width)) * bucketCount))
						);
						updateBucket(bucketIndex, buffer?.valueAt(channelIndex, sampleIndex) ?? 0);
					});

					const bucketWidth = width / bucketCount;
					context.strokeStyle = watcherSeriesColor(channelIndex);
					context.lineWidth = 1;
					context.beginPath();
					for (let bucketIndex = 0; bucketIndex < bucketCount; bucketIndex += 1) {
						if (!bucketHasSample[bucketIndex]) {
							continue;
						}
						const x = bucketIndex * bucketWidth + bucketWidth * 0.5;
						const yMin = valueToY(bucketMin[bucketIndex]);
						const yMax = valueToY(bucketMax[bucketIndex]);
						context.moveTo(x, yMin);
						context.lineTo(x, yMax);
					}
					context.stroke();
					continue;
				}

				context.strokeStyle = watcherSeriesColor(channelIndex);
				context.lineWidth = 1.2;
				context.beginPath();

				let hasPoint = false;
				let currentValue = 0;

				if (latestIndexBeforeWindow !== null) {
					currentValue = buffer.valueAt(channelIndex, latestIndexBeforeWindow);
					context.moveTo(0, valueToY(currentValue));
					hasPoint = true;
				}

				buffer.forEachSince(windowStartMs, (sampleIndex, sampleTimeMs) => {
					const x = ((sampleTimeMs - windowStartMs) / safeWindowMs) * width;
					const nextValue = buffer?.valueAt(channelIndex, sampleIndex) ?? 0;
					const nextY = valueToY(nextValue);

					if (!hasPoint) {
						currentValue = nextValue;
						context.moveTo(x, nextY);
						hasPoint = true;
						return;
					}

					const heldY = valueToY(currentValue);
					context.lineTo(x, heldY);
					context.lineTo(x, nextY);
					currentValue = nextValue;
				});

				if (hasPoint) {
					context.lineTo(width, valueToY(currentValue));
					context.stroke();
				}
			}

			context.strokeStyle = 'rgba(255 255 255 / 0.2)';
			context.lineWidth = 1;
			context.strokeRect(0.5, 0.5, Math.max(0, width - 1), Math.max(0, height - 1));

			if (nowMs - lastRangeSyncAtMs > RANGE_SYNC_INTERVAL_MS) {
				rangeMinLabel = formatWatcherNumber(minValue);
				rangeMaxLabel = formatWatcherNumber(maxValue);
				lastRangeSyncAtMs = nowMs;
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

<div class="watcher-value">
	<div class="watcher-value-meta">
		<div class="legend">
			{#each seriesLabels as label, index}
				<span class="legend-item">
					<span class="legend-dot" style="--curve-color: {watcherSeriesColor(index)}"></span>
					<span>{label}: {formatWatcherNumber(latestValues[index] ?? 0)}{unitSuffix}</span>
				</span>
			{/each}
		</div>
		<div class="range-labels">
			<span>max {rangeMaxLabel}{unitSuffix}</span>
			<span>min {rangeMinLabel}{unitSuffix}</span>
		</div>
	</div>
	<canvas bind:this={canvasElement} class="watcher-value-canvas"></canvas>
</div>

<style>
	.watcher-value {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.watcher-value-meta {
		display: flex;
		justify-content: space-between;
		gap: 0.4rem;
		font-size: 0.65rem;
	}

	.legend {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.45rem;
	}

	.legend-item {
		display: inline-flex;
		align-items: center;
		gap: 0.2rem;
	}

	.legend-dot {
		width: 0.45rem;
		height: 0.45rem;
		border-radius: 100%;
		background: var(--curve-color);
		box-shadow: 0 0 0.3rem color-mix(in oklab, var(--curve-color) 55%, transparent);
	}

	.range-labels {
		display: inline-flex;
		flex-direction: column;
		align-items: flex-end;
		gap: 0.05rem;
		opacity: 0.75;
	}

	.watcher-value-canvas {
		display: block;
		width: 100%;
		height: 8rem;
		border-radius: 0.4rem;
	}
</style>
