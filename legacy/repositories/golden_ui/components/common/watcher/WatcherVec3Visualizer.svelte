<script lang="ts">
	import type { ParamValue, UiParamConstraints } from '../../../types';
	import WatcherProjectionCanvas from './WatcherProjectionCanvas.svelte';
	import { MultiSeriesRingBuffer } from './watcher-buffers';
	import {
		formatWatcherNumber,
		getFixedRange,
		getNowMs,
		type WatcherDecimationMode,
		type WatcherRangeMode
	} from './watcher-utils';

	const RANGE_SYNC_INTERVAL_MS = 120;
	const EPSILON = 1e-6;

	let {
		sampleValue,
		constraints,
		timeWindowMs,
		rangeMode,
		unit,
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

	let latestValue = $state<[number, number, number]>([0, 0, 0]);
	let rangeMin = $state(-1);
	let rangeMax = $state(1);
	let rangeMinLabel = $state('-1');
	let rangeMaxLabel = $state('1');

	let buffer = $state<MultiSeriesRingBuffer | null>(null as MultiSeriesRingBuffer | null);
	let lastSignature = '';

	let unitSuffix = $derived(unit.trim().length > 0 ? ` ${unit.trim()}` : '');

	const computeRange = (nowMs: number): { min: number; max: number } => {
		const safeWindowMs = Math.max(250, timeWindowMs);
		const windowStartMs = nowMs - safeWindowMs;
		const fixedRange = rangeMode === 'fixed' ? getFixedRange(constraints) : null;

		let minValue = Number.POSITIVE_INFINITY;
		let maxValue = Number.NEGATIVE_INFINITY;

		if (fixedRange) {
			minValue = fixedRange.min;
			maxValue = fixedRange.max;
		} else if (buffer && buffer.length > 0) {
			const adaptiveRange = buffer.minMaxSince(windowStartMs);
			if (adaptiveRange) {
				minValue = adaptiveRange.min;
				maxValue = adaptiveRange.max;
			}
		}

		if (!Number.isFinite(minValue) || !Number.isFinite(maxValue)) {
			minValue = Math.min(latestValue[0], latestValue[1], latestValue[2], -1);
			maxValue = Math.max(latestValue[0], latestValue[1], latestValue[2], 1);
		}

		if (Math.abs(maxValue - minValue) <= EPSILON) {
			const center = maxValue;
			const padding = Math.max(Math.abs(center) * 0.2, 0.5);
			minValue = center - padding;
			maxValue = center + padding;
		}

		return { min: minValue, max: maxValue };
	};

	$effect(() => {
		streamKey;
		buffer = null;
		lastSignature = '';
		latestValue = [0, 0, 0];
		rangeMin = -1;
		rangeMax = 1;
		rangeMinLabel = '-1';
		rangeMaxLabel = '1';
	});

	$effect(() => {
		if (sampleValue.kind !== 'vec3') {
			return;
		}

		if (!buffer || buffer.seriesCount !== 3) {
			buffer = new MultiSeriesRingBuffer(3, 16384);
			lastSignature = '';
		}

		const x = Number(sampleValue.value[0] ?? 0);
		const y = Number(sampleValue.value[1] ?? 0);
		const z = Number(sampleValue.value[2] ?? 0);
		const signature = `${x}|${y}|${z}`;
		if (signature === lastSignature) {
			return;
		}

		lastSignature = signature;
		latestValue = [x, y, z];
		buffer.push(getNowMs(), [x, y, z]);
	});

	$effect(() => {
		timeWindowMs;
		rangeMode;
		constraints.range;
		latestValue;
		buffer;

		if (typeof window === 'undefined') {
			const initialRange = computeRange(getNowMs());
			rangeMin = initialRange.min;
			rangeMax = initialRange.max;
			rangeMinLabel = formatWatcherNumber(initialRange.min);
			rangeMaxLabel = formatWatcherNumber(initialRange.max);
			return;
		}

		let disposed = false;
		let animationFrameId = 0;
		let lastSyncAtMs = Number.NEGATIVE_INFINITY;

		const frame = (nowMs: number): void => {
			if (disposed) {
				return;
			}
			if (nowMs - lastSyncAtMs >= RANGE_SYNC_INTERVAL_MS) {
				const nextRange = computeRange(nowMs);
				rangeMin = nextRange.min;
				rangeMax = nextRange.max;
				rangeMinLabel = formatWatcherNumber(nextRange.min);
				rangeMaxLabel = formatWatcherNumber(nextRange.max);
				lastSyncAtMs = nowMs;
			}
			animationFrameId = window.requestAnimationFrame(frame);
		};

		animationFrameId = window.requestAnimationFrame(frame);

		return () => {
			disposed = true;
			window.cancelAnimationFrame(animationFrameId);
		};
	});
</script>

<div class="watcher-vec3">
	<div class="watcher-vec3-meta">
		<span>X {formatWatcherNumber(latestValue[0])}{unitSuffix}</span>
		<span>Y {formatWatcherNumber(latestValue[1])}{unitSuffix}</span>
		<span>Z {formatWatcherNumber(latestValue[2])}{unitSuffix}</span>
		<span class="range">range [{rangeMinLabel}, {rangeMaxLabel}]{unitSuffix}</span>
	</div>

	<div class="watcher-vec3-grid">
		<div class="projection">
			<WatcherProjectionCanvas
				{buffer}
				xChannel={0}
				yChannel={1}
				{timeWindowMs}
				{rangeMin}
				{rangeMax}
				color="#ff5500"
				latestPoint={[latestValue[0], latestValue[1]]}
				axisLabel="X / Y" />
		</div>
		<div class="projection">
			<WatcherProjectionCanvas
				{buffer}
				xChannel={0}
				yChannel={2}
				{timeWindowMs}
				{rangeMin}
				{rangeMax}
				color="#aaff00"
				latestPoint={[latestValue[0], latestValue[2]]}
				axisLabel="X / Z" />
		</div>
		<div class="projection">
			<WatcherProjectionCanvas
				{buffer}
				xChannel={2}
				yChannel={1}
				{timeWindowMs}
				{rangeMin}
				{rangeMax}
				color="#00aaff"
				latestPoint={[latestValue[2], latestValue[1]]}
				axisLabel="Z / Y" />
		</div>
	</div>
</div>

<style>
	.watcher-vec3 {
		display: flex;
		flex-direction: column;
		gap: 0.35rem;
	}

	.watcher-vec3-meta {
		display: inline-flex;
		flex-wrap: wrap;
		gap: 0.5rem;
		font-size: 0.65rem;
	}

	.watcher-vec3-meta .range {
		opacity: 0.75;
	}

	.watcher-vec3-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 0.3rem;
	}

	.watcher-vec3-grid .projection {
		width: 100%;
		height: 7rem;
	}
</style>
