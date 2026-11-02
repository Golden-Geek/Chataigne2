<script lang="ts">
	import { onDestroy, untrack } from 'svelte';
	import { appState, IconButton, type UiNodeDto } from 'golden_ui';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import referenceIcon from 'golden_ui/style/icons/parameter/reference.svg';
	import { useDocumentSelection } from '../selection.svelte';
	import { CHANNEL_PAGE_SIZE, ChannelPageReader, channelPageState } from './channel-page';
	import { invokeMediaEdit } from './operations';
	import { MEDIA_NAMESPACE } from './generated/constants';
	let { node }: { node: UiNodeDto } = $props();
	const session = $derived(appState.session);
	const selection = useDocumentSelection();
	const view = $state(channelPageState());
	const reader = new ChannelPageReader(() => session, view, invokeMediaEdit);
	let page = $state(0);
	const channels = $derived(reader.page);
	const pages = $derived(Math.max(1, Math.ceil((channels?.total ?? 0) / CHANNEL_PAGE_SIZE)));
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
		if (channels && page >= pages) page = pages - 1;
	});
	onDestroy(() => reader.dispose());
</script>

<section
	class="reference-channels"
	aria-label="Channels"
	aria-busy={view.loading}
	data-channel-count={channels?.total}
	data-media={node.uuid}>
	<header>
		<span>Channels</span><span class="spacer"></span>
		{#if channels && pages > 1}
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
				disabled={busy || page + 1 >= pages}
				onclick={() => {
					page += 1;
				}} />
		{/if}
	</header>
	{#if view.error}<p role="alert">{view.error}</p>
	{:else if channels}
		{#if channels.status === 'Unconfigured'}<p role="status">Choose a target.</p>
		{:else if channels.status === 'MissingTarget'}<p role="status">Target unavailable.</p>
		{:else if channels.status === 'MissingMedia'}<p role="status">Media no longer exists.</p>
		{:else if channels.total === 0}<p role="status">No channels.</p>
		{:else}
			<ul aria-label="Mirrored channels">
				{#each channels.channels as channel (channel.reference.channel)}
					<li data-channel={channel.reference.channel}>
						<span class="label">{channel.label}</span>
						<IconButton
							icon={referenceIcon}
							label={`Edit target channel ${channel.label}`}
							disabled={busy}
							onclick={() => selection.select(channel.reference.channel)} />
					</li>
				{/each}
			</ul>
		{/if}
	{:else}<p role="status">{view.loading ? 'Loading channels…' : 'Channels unavailable.'}</p>{/if}
</section>

<style>
	.reference-channels {
		min-width: 0;
	}
	header,
	li {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.5rem;
	}
	header {
		font-weight: 600;
	}
	.spacer,
	.label {
		flex: 1;
	}
	.label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.page {
		font-size: 0.85em;
		font-weight: 400;
	}
	ul {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	p {
		margin: 0;
		padding: 0.5rem;
		font-size: 0.85em;
	}
	:global(.reference-channels .previous img) {
		transform: rotate(180deg);
	}
</style>
