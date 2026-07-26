<script lang="ts">
	import { onDestroy, onMount } from 'svelte';
	import type { ChannelObservation } from 'golden_audio_ui';
	import { CanvasRenderScheduler } from './canvas-render-scheduler';

	let { inputs, outputs, channelLabels } = $props<{
		inputs: readonly ChannelObservation[];
		outputs: readonly ChannelObservation[];
		channelLabels: ReadonlyMap<string, string>;
	}>();

	let canvas: HTMLCanvasElement | null = $state(null);
	let canvasWidth = $state(0);
	let canvasHeight = $state(0);
	let renderReady = $state(false);
	let scheduler: CanvasRenderScheduler | null = null;
	let resizeObserver: ResizeObserver | null = null;

	const label = (channel: string): string =>
		channelLabels.get(channel) ?? `Channel ${channel.slice(0, 8)}`;

	const normalized = (dbfs: number): number =>
		Math.max(0, Math.min(1, (Math.max(-60, dbfs) + 60) / 60));

	const drawBank = (
		context: CanvasRenderingContext2D,
		title: string,
		channels: readonly ChannelObservation[],
		x: number,
		width: number
	): void => {
		context.fillStyle = '#aeb8c7';
		context.font = '600 12px system-ui, sans-serif';
		context.fillText(`${title} (${channels.length})`, x, 15);
		if (channels.length === 0) {
			context.fillStyle = '#748096';
			context.font = '11px system-ui, sans-serif';
			context.fillText('No meter data', x, 36);
			return;
		}
		const availableHeight = Math.max(1, canvasHeight - 27);
		const rowHeight = availableHeight / channels.length;
		const textWidth = Math.min(width * 0.42, 112);
		const meterX = x + textWidth;
		const meterWidth = Math.max(1, width - textWidth - 8);
		for (let index = 0; index < channels.length; index += 1) {
			const channel = channels[index];
			const y = 24 + index * rowHeight;
			const barHeight = Math.max(2, Math.min(10, rowHeight - 3));
			context.fillStyle = channel.clipped ? '#ff8e8e' : '#d9e0ea';
			context.font = '10px system-ui, sans-serif';
			context.fillText(label(channel.channel), x, y + barHeight);
			context.fillStyle = '#202736';
			context.fillRect(meterX, y, meterWidth, barHeight);
			const gradient = context.createLinearGradient(meterX, 0, meterX + meterWidth, 0);
			gradient.addColorStop(0, '#50c98a');
			gradient.addColorStop(0.82, '#e3b153');
			gradient.addColorStop(1, '#ef6d6d');
			context.fillStyle = gradient;
			context.fillRect(meterX, y, meterWidth * normalized(channel.rms_dbfs), barHeight);
			context.fillStyle = '#f3f6fa';
			context.fillRect(meterX + meterWidth * normalized(channel.peak_dbfs), y, 1.2, barHeight);
		}
	};

	const draw = (): void => {
		if (!canvas || canvasWidth <= 0 || canvasHeight <= 0) return;
		const context = canvas.getContext('2d');
		if (!context) return;
		const ratio = Math.max(1, globalThis.devicePixelRatio ?? 1);
		const width = Math.max(1, Math.round(canvasWidth * ratio));
		const height = Math.max(1, Math.round(canvasHeight * ratio));
		if (canvas.width !== width) canvas.width = width;
		if (canvas.height !== height) canvas.height = height;
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		context.clearRect(0, 0, canvasWidth, canvasHeight);
		context.fillStyle = '#141a25';
		context.fillRect(0, 0, canvasWidth, canvasHeight);
		const gap = 14;
		const bankWidth = Math.max(1, (canvasWidth - gap) / 2);
		drawBank(context, 'Inputs', inputs, 0, bankWidth);
		drawBank(context, 'Outputs', outputs, bankWidth + gap, bankWidth);
	};

	$effect(() => {
		inputs;
		outputs;
		channelLabels;
		canvasWidth;
		canvasHeight;
		renderReady;
		scheduler?.request(draw);
	});

	onMount(() => {
		if (!canvas) return;
		scheduler = new CanvasRenderScheduler(requestAnimationFrame, cancelAnimationFrame);
		resizeObserver = new ResizeObserver(([entry]) => {
			canvasWidth = entry.contentRect.width;
			canvasHeight = entry.contentRect.height;
		});
		resizeObserver.observe(canvas);
		renderReady = true;
	});

	onDestroy(() => {
		resizeObserver?.disconnect();
		scheduler?.dispose();
	});
</script>

<section class="meter-bank" aria-labelledby="sound-card-live-meters-heading">
	<header>
		<h3 id="sound-card-live-meters-heading">Live meters</h3>
		<span>{inputs.length + outputs.length} channels</span>
	</header>
	<canvas bind:this={canvas} aria-label="Sound Card input and output meters">
		Live Sound Card meters for {inputs.length} input and {outputs.length} output channels.
	</canvas>

	<table class="visually-hidden">
		<caption>Accessible Sound Card meter values</caption>
		<thead>
			<tr>
				<th scope="col">Direction</th>
				<th scope="col">Channel</th>
				<th scope="col">RMS</th>
				<th scope="col">Peak</th>
				<th scope="col">Clipped</th>
			</tr>
		</thead>
		<tbody>
			{#each inputs as channel (channel.channel)}
				<tr>
					<td>Input</td>
					<td>{label(channel.channel)}</td>
					<td>{channel.rms_dbfs.toFixed(1)} dBFS</td>
					<td>{channel.peak_dbfs.toFixed(1)} dBFS</td>
					<td>{channel.clipped ? 'Yes' : 'No'}</td>
				</tr>
			{/each}
			{#each outputs as channel (channel.channel)}
				<tr>
					<td>Output</td>
					<td>{label(channel.channel)}</td>
					<td>{channel.rms_dbfs.toFixed(1)} dBFS</td>
					<td>{channel.peak_dbfs.toFixed(1)} dBFS</td>
					<td>{channel.clipped ? 'Yes' : 'No'}</td>
				</tr>
			{/each}
		</tbody>
	</table>
</section>

<style>
	.meter-bank {
		display: grid;
		gap: 0.5rem;
		min-inline-size: 0;
		padding: 0.65rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.4rem;
		background: var(--gc-color-bg-light);
	}

	header {
		display: flex;
		align-items: baseline;
		justify-content: space-between;
		gap: 0.6rem;
	}

	h3 {
		margin: 0;
		font-size: 0.85rem;
	}

	header span {
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	canvas {
		display: block;
		inline-size: 100%;
		block-size: min(18rem, 36vh);
		min-block-size: 8rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: #141a25;
	}

	.visually-hidden {
		position: absolute;
		inline-size: 0.0625rem;
		block-size: 0.0625rem;
		padding: 0;
		margin: -0.0625rem;
		overflow: hidden;
		clip: rect(0, 0, 0, 0);
		white-space: nowrap;
		border: 0;
	}
</style>
