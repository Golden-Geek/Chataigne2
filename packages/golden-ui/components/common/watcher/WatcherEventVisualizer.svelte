<script lang="ts">
	import type { ParamValue, UiParamConstraints } from '../../../types';
	import { TimeRingBuffer } from './watcher-buffers';
	import {
		extractEventLabel,
		getNowMs,
		type WatcherDecimationMode,
		type WatcherRangeMode,
		watcherEventColor
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
	const METRICS_SYNC_INTERVAL_MS = 180;

	let canvasElement = $state<HTMLCanvasElement | null>(null);
	let latestLabel = $state('-');
	let eventCount = $state(0);
	let recentEvents = $state<Array<{ label: string; timeMs: number }>>([]);

	let eventBuffer: TimeRingBuffer<string> | null = null;
	let lastLabel = '';
	let lastMetricsSyncAtMs = 0;

	const readRecentEvents = (
		buffer: TimeRingBuffer<string>,
		limit = 6
	): Array<{ label: string; timeMs: number }> => {
		const recent: Array<{ label: string; timeMs: number }> = [];
		buffer.forEachRecent(limit, (sampleTimeMs, label) => {
			recent.push({ label, timeMs: sampleTimeMs });
		});
		return recent;
	};

	$effect(() => {
		streamKey;
		eventBuffer = null;
		lastLabel = '';
		latestLabel = '-';
		eventCount = 0;
		recentEvents = [];
	});

	$effect(() => {
		const nextLabel = extractEventLabel(sampleValue);
		if (!nextLabel) {
			return;
		}

		if (!eventBuffer) {
			eventBuffer = new TimeRingBuffer<string>(8192);
		}

		const isTrigger = sampleValue.kind === 'trigger';
		if (!isTrigger && nextLabel === lastLabel) {
			return;
		}

		const nowMs = getNowMs();
		lastLabel = nextLabel;
		latestLabel = nextLabel;
		eventBuffer.push(nowMs, nextLabel);
		recentEvents = readRecentEvents(eventBuffer, 6);
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

			if (!eventBuffer || eventBuffer.length === 0) {
				drawEmptyState(context, width, height);
				return;
			}

			const centerY = height * 0.5;
			context.strokeStyle = 'rgba(255 255 255 / 0.08)';
			context.lineWidth = 1;
			context.beginPath();
			context.moveTo(0, centerY);
			context.lineTo(width, centerY);
			context.stroke();

			let countedEvents = 0;
			eventBuffer.forEachSince(windowStartMs, (sampleTimeMs, label) => {
				countedEvents += 1;
				const x = ((sampleTimeMs - windowStartMs) / safeWindowMs) * width;
				const age = (nowMs - sampleTimeMs) / safeWindowMs;
				const alpha = Math.max(0.2, 1 - age * 0.85);

				context.strokeStyle = watcherEventColor(label);
				context.globalAlpha = alpha;
				context.lineWidth = 1.2;
				context.beginPath();
				context.moveTo(x, height * 0.08);
				context.lineTo(x, height * 0.92);
				context.stroke();
			});
			context.globalAlpha = 1;

			context.strokeStyle = 'rgba(255 255 255 / 0.2)';
			context.lineWidth = 1;
			context.strokeRect(0.5, 0.5, Math.max(0, width - 1), Math.max(0, height - 1));

			if (nowMs - lastMetricsSyncAtMs > METRICS_SYNC_INTERVAL_MS) {
				eventCount = countedEvents;
				lastMetricsSyncAtMs = nowMs;
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

<div class="watcher-events">
	<div class="watcher-events-meta">
		<span class="latest-label">latest: {latestLabel}</span>
		<span class="event-count">{eventCount} event{eventCount === 1 ? '' : 's'}</span>
	</div>

	<canvas bind:this={canvasElement} class="watcher-events-canvas"></canvas>

	{#if recentEvents.length > 0}
		<div class="recent-events">
			{#each recentEvents as eventEntry}
				<span
					class="recent-event-chip"
					style="--event-color: {watcherEventColor(eventEntry.label)}"
					title={eventEntry.label}>
					{eventEntry.label}
				</span>
			{/each}
		</div>
	{/if}
</div>

<style>
	.watcher-events {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.watcher-events-meta {
		display: flex;
		justify-content: space-between;
		font-size: 0.65rem;
		gap: 0.3rem;
	}

	.latest-label {
		font-weight: 600;
	}

	.event-count {
		opacity: 0.75;
	}

	.watcher-events-canvas {
		display: block;
		width: 100%;
		height: 6rem;
		border-radius: 0.4rem;
	}

	.recent-events {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.3rem;
	}

	.recent-event-chip {
		font-size: 0.62rem;
		line-height: 1;
		padding: 0.2rem 0.35rem;
		border-radius: 0.5rem;
		background-color: color-mix(in oklab, var(--event-color) 30%, transparent);
		border: solid 0.06rem color-mix(in oklab, var(--event-color) 60%, transparent);
		color: color-mix(in oklab, var(--event-color) 85%, white);
		max-width: 9rem;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
