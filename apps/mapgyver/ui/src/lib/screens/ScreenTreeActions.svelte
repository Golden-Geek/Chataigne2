<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, type UiNodeDto, type WorkbenchSession } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import { useVideoStatus } from '../runtime/context.svelte';
	import { invokeScreenEdit } from './operations';
	import { SCREEN_NAMESPACE } from './generated/constants';
	import { MEDIA_NAMESPACE } from '../media/generated/constants';
	import { createRowState, ScreenRowReader } from './row-reader';
	let { node }: { node: UiNodeDto } = $props();
	const state = $state(createRowState());
	const video = useVideoStatus();
	const reader = new ScreenRowReader<WorkbenchSession>(state, async (session, node) => {
		const result = await invokeScreenEdit(session, { kind: 'InspectRow', node });
		if (result.kind !== 'Row') throw new Error('Unexpected Screen row response');
		return result.snapshot;
	});
	$effect(() => {
		const session = appState.session;
		if (session) {
			void session.status;
			void session.loading.visible;
			void session.snapshotGeneration;
			if (!session.hasActiveEditSession) void session.currentHistoryStateId;
			void session.editSessionEpoch;
			void session.getCustomEventSequence(SCREEN_NAMESPACE);
			void session.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		const uuid = node.uuid;
		untrack(() => reader.refresh(session, uuid));
	});
	onDestroy(() => reader.dispose());
	const row = $derived(state.snapshot);
	const runtime = $derived(video.status?.screens.find((screen) => screen.screen === node.uuid));
	const extent = $derived(runtime?.extent ?? row?.extent);
	const title = $derived(
		[
			row?.detail,
			extent ? `${extent.width} × ${extent.height}` : '',
			row?.enabled === false ? 'Disabled' : '',
			runtime ? `${runtime.state}; API-accepted output FPS, not display scanout` : '',
			state.error
		]
			.filter(Boolean)
			.join('\n')
	);
</script>

<span
	class="row-details"
	class:screen={node.node_type === 'mapgyver.screen'}
	aria-label={node.node_type === 'mapgyver.screen'
		? 'Screen output details'
		: 'Surface source details'}
	{title}>
	<span class="detail">{row?.detail ?? (state.error ? 'Unavailable' : '…')}</span>
	{#if extent}<span class="resolution">{extent.width} × {extent.height}</span>{/if}
	{#if node.node_type === 'mapgyver.screen'}<span class="fps"
			>{runtime?.accepted_fps == null ? '—' : runtime.accepted_fps.toFixed(1)} fps</span
		>{/if}
</span>
{#if node.node_type === 'mapgyver.screen'}
	<NodeAddButton
		{node}
		disabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession} />
{/if}

<style>
	.row-details {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		min-width: 0;
		max-width: 24rem;
		font-size: 0.7rem;
		color: var(--gc-color-text-muted, var(--gc-color-text));
		font-variant-numeric: tabular-nums;
	}
	.detail {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.resolution,
	.fps {
		white-space: nowrap;
	}
	@container (max-width: 28rem) {
		.detail {
			max-width: 8rem;
		}
	}
	@container (max-width: 20rem) {
		.screen .detail,
		.screen .resolution {
			display: none;
		}
	}
</style>
