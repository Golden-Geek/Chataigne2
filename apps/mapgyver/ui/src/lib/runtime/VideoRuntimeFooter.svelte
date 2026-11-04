<script lang="ts">
	import { useVideoStatus } from './context.svelte';
	import { formatVideoMemory } from './format';
	const state = useVideoStatus();
	const status = $derived(state.status);
	const title = $derived(
		[
			status?.gpu ? `Renderer adapter: ${status.gpu}` : 'No renderer adapter observation',
			status?.gpu_restart_required ? 'GPU preference changed. Restart MapGyver to apply.' : '',
			'Renderer FPS counts successful API frame calls, not display presentation.',
			'CPU frame call time excludes asynchronous GPU work and presentation.',
			'Tracked bytes are renderer allocation charges, not physical VRAM residency.',
			status?.resources
				? `${status.resources.in_flight} in flight, ${status.resources.retiring} retiring`
				: '',
			status?.error ?? state.error
		]
			.filter(Boolean)
			.join('\n')
	);
</script>

<output
	class="video-runtime"
	aria-label="Video runtime"
	data-state={status?.state ?? state.connection}
	data-sequence={status?.sequence}
	{title}>
	{#if status?.state === 'Running'}
		<span>{status.submitted_fps === null ? '—' : status.submitted_fps.toFixed(1)} fps</span>
		<span class="gpu">{status.gpu ?? 'GPU —'}</span>
		<span>{status.cpu_submit_ms === null ? '—' : status.cpu_submit_ms.toFixed(2)} ms CPU</span>
		{#if status.resources}<span>{formatVideoMemory(status.resources.used_bytes)}</span>{/if}
	{:else}
		<span
			>Video {status?.state.toLowerCase() ??
				(state.error
					? 'recovering'
					: state.connection === 'waiting'
						? 'starting'
						: 'unavailable')}</span>
	{/if}
	{#if status?.gpu_restart_required}<span>GPU restart required</span>{/if}
</output>

<style>
	.video-runtime {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		flex: 0 1 auto;
		gap: 0.6rem;
		min-width: 0;
		font-size: 0.68rem;
		color: var(--gc-color-text);
		font-variant-numeric: tabular-nums;
		overflow: hidden;
	}
	span {
		white-space: nowrap;
		flex: 0 0 auto;
	}
	.gpu {
		min-width: 0;
		flex: 0 3 auto;
		max-width: 10rem;
		overflow: hidden;
		text-overflow: clip;
	}
	@media (max-width: 48rem) {
		.gpu {
			display: none;
		}
	}
</style>
