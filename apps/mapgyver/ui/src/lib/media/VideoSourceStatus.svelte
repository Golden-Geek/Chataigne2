<script lang="ts">
	import { untrack } from 'svelte';
	import { appState } from 'golden_ui';
	import type { ChannelReference } from './generated/ChannelReference';
	import type { AssetLocator } from './generated/AssetLocator';
	import { invokeMediaEdit } from './operations';
	import { createVideoSourceState, VideoSourceReader } from './video-source-reader';
	let { reference, file }: { reference: ChannelReference; file: AssetLocator | null } = $props();
	const state = $state(createVideoSourceState());
	const session = $derived(appState.session);
	const status = $derived(state.status);
	const title = $derived(
		[
			status?.state === 'Prepared' ? 'Timing is known; this source is not being rendered.' : '',
			status?.error?.message ?? state.error,
			status?.retrying ? 'Retrying automatically.' : ''
		]
			.filter(Boolean)
			.join('\n')
	);
	$effect(() => {
		const current = session;
		const generation = current?.snapshotGeneration;
		void current?.status;
		void current?.loading.visible;
		void JSON.stringify(file);
		const selected = { ...reference };
		const reader = new VideoSourceReader(
			state,
			async () => {
				if (!current || current.hasActiveEditSession) return null;
				const result = await invokeMediaEdit(current, {
					kind: 'InspectVideoSource',
					reference: selected
				});
				if (result.kind !== 'VideoSource') throw new Error('Unexpected Video source response');
				return result.status;
			},
			() =>
				session === current &&
				current?.status === 'connected' &&
				current.snapshotGeneration === generation &&
				!current.loading.visible
		);
		return untrack(() => reader.start());
	});
</script>

<output
	class="source-status"
	aria-label="Video source status"
	{title}
	data-state={status?.state ?? 'Waiting'}>
	{status?.state === 'Prepared'
		? 'Metadata ready'
		: (status?.state ?? (state.error ? 'Unavailable' : 'Waiting'))}
	{#if status?.duration_ms != null}<span>· {(status.duration_ms / 1000).toFixed(3)} s</span>{/if}
	{#if status?.retrying}<span>· retrying</span>{/if}
	{#if status?.error}<span class="reason">{status.error.message}</span>{/if}
</output>

<style>
	.source-status {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 0.3em;
		color: var(--gc-color-text-muted, var(--gc-color-text));
		font-size: 0.85em;
		min-width: 0;
	}
	.reason {
		flex-basis: 100%;
		overflow-wrap: anywhere;
	}
</style>
