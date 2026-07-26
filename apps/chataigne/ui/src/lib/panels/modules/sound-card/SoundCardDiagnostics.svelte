<script lang="ts">
	import type { SoundCardUiTelemetryDto } from '$lib/modules/audio/sound-card/generated';

	let { telemetry, xruns, lastError } = $props<{
		telemetry: SoundCardUiTelemetryDto;
		xruns: number | null;
		lastError: string | null;
	}>();
</script>

<dl class="diagnostics">
	<div>
		<dt>XRuns</dt>
		<dd>{xruns ?? 'Unavailable'}</dd>
	</div>
	<div>
		<dt>Dropped events</dt>
		<dd>{telemetry.dropped_event_count}</dd>
	</div>
	<div>
		<dt>Queue pressure</dt>
		<dd>{telemetry.queue_pressure_count}</dd>
	</div>
	<div>
		<dt>Playback queue pressure</dt>
		<dd>{telemetry.playback.command_queue_pressure_count}</dd>
	</div>
	<div>
		<dt>Render timing</dt>
		<dd>
			{telemetry.runtime.maximum_render_time_micros} us max /
			{telemetry.runtime.rendered_blocks} blocks
		</dd>
	</div>
	<div>
		<dt>Render deadline misses</dt>
		<dd>{telemetry.runtime.deadline_miss_count}</dd>
	</div>
	<div>
		<dt>Callback XRuns</dt>
		<dd>
			{telemetry.runtime.xrun_count} total /
			{telemetry.runtime.input_underflow_count} input under /
			{telemetry.runtime.output_underflow_count} output under
		</dd>
	</div>
	<div>
		<dt>Render control pressure</dt>
		<dd>{telemetry.runtime.control_queue_pressure_count}</dd>
	</div>
	<div>
		<dt>Resident playback cache</dt>
		<dd>
			{telemetry.playback.cache_entries} assets /
			{(telemetry.playback.resident_bytes / 1_048_576).toFixed(1)} MiB
		</dd>
	</div>
	<div>
		<dt>Analysis frames</dt>
		<dd>
			{telemetry.analysis.diagnostics.processed_frames} processed /
			{telemetry.analysis.diagnostics.dropped_frames} dropped
		</dd>
	</div>
	<div>
		<dt>Analysis worker</dt>
		<dd>
			{telemetry.analysis.diagnostics.worker_time_micros} µs current /
			{telemetry.analysis.diagnostics.maximum_worker_time_micros} µs max
		</dd>
	</div>
	<div>
		<dt>Render frame</dt>
		<dd>{telemetry.render_frame}</dd>
	</div>
</dl>

{#if lastError}
	<details class="last-error">
		<summary>Last backend error</summary>
		<pre>{lastError}</pre>
	</details>
{/if}

<style>
	.diagnostics {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(min(11rem, 100%), 1fr));
		gap: 0.55rem;
		margin: 0;
	}

	.diagnostics div {
		padding: 0.55rem;
		border: 0.0625rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: var(--gc-color-bg-light);
	}

	dt {
		color: var(--gc-color-text-muted);
		font-size: 0.7rem;
	}

	dd {
		margin: 0.2rem 0 0;
		font-size: 0.82rem;
		font-variant-numeric: tabular-nums;
	}

	.last-error {
		margin-block-start: 0.65rem;
		padding: 0.55rem;
		border: 0.0625rem solid #8e4444;
		border-radius: 0.35rem;
		background: #3c2328;
	}

	summary {
		cursor: pointer;
	}

	pre {
		max-block-size: 12rem;
		overflow: auto;
		white-space: pre-wrap;
	}
</style>
