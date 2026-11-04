<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { CHANNEL_PAGE_SIZE, ChannelPageReader, channelPageState } from './channel-page';
	import { invokeMediaEdit } from './operations';
	import { MEDIA_NAMESPACE } from './generated/constants';
	let {
		node,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	let page = $state(0);
	const session = $derived(appState.session);
	const view = $state(channelPageState());
	const reader = new ChannelPageReader(() => session, view, invokeMediaEdit);
	const channels = $derived(reader.page);
	// Keep only admitted membership while edits refresh metadata, so live Parameter editors
	// retain their DOM and input capture. Definitions still come from the live Golden nodes.
	let membership = $state.raw<{
		session: typeof session;
		generation: number;
		media: string;
		start: number;
		total: number;
		ids: Set<string>;
	} | null>(null);
	const admitted = $derived(
		session?.status === 'connected' &&
			!session.loading.visible &&
			membership?.session === session &&
			membership.generation === session.snapshotGeneration &&
			membership.media === node.uuid &&
			membership.start === page * CHANNEL_PAGE_SIZE
			? membership
			: null
	);
	const pages = $derived(
		Math.max(1, Math.ceil((channels?.total ?? admitted?.total ?? 0) / CHANNEL_PAGE_SIZE))
	);
	const visible = $derived(admitted?.ids ?? new Set<string>());
	const busy = $derived(
		view.loading ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
	);
	$effect(() => {
		const media = node.uuid,
			start = page * CHANNEL_PAGE_SIZE,
			current = session;
		if (current) {
			void current.status;
			void current.loading.visible;
			void current.snapshotGeneration;
			void current.currentHistoryStateId;
			void current.hasActiveEditSession;
			void current.editSessionEpoch;
			void current.getCustomEventSequence(MEDIA_NAMESPACE);
		}
		untrack(() => reader.refresh(media, start));
	});
	$effect(() => {
		if (channels && session)
			membership = {
				session,
				generation: session.snapshotGeneration,
				media: node.uuid,
				start: channels.start,
				total: channels.total,
				ids: new Set(channels.channels.map((channel) => channel.reference.channel))
			};
		if (channels && page >= pages) page = pages - 1;
	});
	onDestroy(() => reader.dispose());
</script>

<section
	class="channels-section"
	aria-label="Channels"
	aria-busy={view.loading}
	data-channel-count={admitted?.total}>
	<header>
		<span>Channels</span>
		<span class="spacer"></span>
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous channels"
				class="previous"
				disabled={busy || page === 0}
				onclick={() => {
					page -= 1;
				}} />
			<span class="page">{page + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next channels"
				disabled={busy || page + 1 === pages}
				onclick={() => {
					page += 1;
				}} />
		{/if}
		<NodeAddButton {node} disabled={busy} />
	</header>
	{#if view.error}<p role="alert">{view.error}</p>
	{:else if !admitted}<p role="status">
			{view.loading ? 'Loading channels…' : 'Channels unavailable.'}
		</p>{/if}
	{@render defaultChildren?.('', (child) => visible.has(child.uuid))}
</section>

<style>
	.channels-section {
		min-width: 0;
	}
	header {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.5rem;
		font-weight: 600;
	}
	.spacer {
		flex: 1;
	}
	.page {
		font-size: 0.85em;
		font-weight: 400;
	}
	:global(.channels-section .previous img) {
		transform: rotate(180deg);
	}
</style>
