<script lang="ts">
	import type { Snippet } from 'svelte';
	import { appState, IconButton, type UiNodeDto, type NodeInspectorChildFilter } from 'golden_ui';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import scriptIcon from 'golden_ui/style/icons/node/script.svg';
	let {
		node,
		defaultChildren,
		oncreate,
		disabled
	}: {
		node: UiNodeDto;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
		oncreate: () => void | Promise<void>;
		disabled: boolean;
	} = $props();
	let page = $state(0);
	const PAGE_SIZE = 32;
	const scripts = $derived(
		node.children.filter((id) => appState.session?.getNodeData(id)?.node_type === 'script')
	);
	const pages = $derived(Math.max(1, Math.ceil(scripts.length / PAGE_SIZE)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const visible = $derived(
		new Set(scripts.slice(currentPage * PAGE_SIZE, (currentPage + 1) * PAGE_SIZE))
	);
</script>

<section class="media-scripts-section" aria-label="Media scripts">
	<header>
		<span>Scripts</span><span class="spacer"></span>
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous scripts"
				class="previous"
				disabled={disabled || currentPage === 0}
				onclick={() => {
					page = currentPage - 1;
				}} />
			<span>{currentPage + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next scripts"
				disabled={disabled || currentPage + 1 === pages}
				onclick={() => {
					page = currentPage + 1;
				}} />
		{/if}
		<IconButton icon={scriptIcon} label="Add Media Script" {disabled} onclick={oncreate} />
	</header>
	{@render defaultChildren?.('', (child) => visible.has(child.node_id))}
</section>

<style>
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
	.media-scripts-section :global(.previous img) {
		transform: rotate(180deg);
	}
</style>
