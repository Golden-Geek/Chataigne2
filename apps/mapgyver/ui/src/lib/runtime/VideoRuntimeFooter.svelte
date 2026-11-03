<script lang="ts">
	import { onMount } from 'svelte';
	import { createDesktopVideoStatusTransport } from './transport';
	import { createVideoStatusState, VideoStatusController } from './status';
	const state = $state(createVideoStatusState());
	const controller = new VideoStatusController(createDesktopVideoStatusTransport(), state);
	onMount(() => controller.connect());
	const status = $derived(state.status);
	const title = $derived(
		[
			status?.gpu ? `Renderer adapter: ${status.gpu}` : 'No renderer adapter observation',
			'Submitted FPS measures successful renderer submissions, not display presentation.',
			'CPU submit time excludes asynchronous GPU work and presentation.',
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
		<span>{status.cpu_submit_ms === null ? '—' : status.cpu_submit_ms.toFixed(2)} ms submit</span>
		{#if status.resources}<span>{(status.resources.used_bytes / 1048576).toFixed(1)} MiB</span>{/if}
	{:else}
		<span
			>Video {status?.state.toLowerCase() ??
				(state.connection === 'waiting' ? 'starting' : 'unavailable')}</span>
	{/if}
</output>

<style>
	.video-runtime {
		display: flex;
		align-items: center;
		gap: 0.6rem;
		min-width: 0;
		font-size: 0.68rem;
		color: var(--gc-color-text);
		font-variant-numeric: tabular-nums;
		overflow: hidden;
	}
	span {
		white-space: nowrap;
	}
	.gpu {
		max-width: 10rem;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
