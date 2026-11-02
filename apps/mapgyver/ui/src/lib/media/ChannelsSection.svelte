<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	let {
		node,
		defaultChildren
	}: {
		node: UiNodeDto;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
	} = $props();
	const PAGE_SIZE = 32;
	let page = $state(0);
	const session = $derived(appState.session);
	const channels = $derived(
		node.children.filter((id) => session?.getNodeData(id)?.node_type === 'mapgyver.media.channel')
	);
	const pages = $derived(Math.max(1, Math.ceil(channels.length / PAGE_SIZE)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const visible = $derived(
		new Set(channels.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE))
	);
	const busy = $derived(
		session?.status !== 'connected' || session.loading.visible || session.hasActiveEditSession
	);
</script>

<section class="channels-section" aria-label="Channels" data-channel-count={channels.length}>
	<header>
		<span>Channels</span>
		<span class="spacer"></span>
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous channels"
				class="previous"
				disabled={busy || currentPage === 0}
				onclick={() => {
					page = currentPage - 1;
				}} />
			<span class="page">{currentPage + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next channels"
				disabled={busy || currentPage + 1 === pages}
				onclick={() => {
					page = currentPage + 1;
				}} />
		{/if}
		<NodeAddButton {node} disabled={busy} />
	</header>
	{@render defaultChildren?.('', (child) => visible.has(child.node_id))}
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
