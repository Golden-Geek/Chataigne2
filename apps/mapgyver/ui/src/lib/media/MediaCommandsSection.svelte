<script lang="ts">
	import { onDestroy, untrack, type Snippet } from 'svelte';
	import {
		appState,
		IconButton,
		type UiNodeDto,
		type UiCreatableUserItem,
		type NodeInspectorChildFilter
	} from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import arrowIcon from 'golden_ui/style/icons/arrow.svg';
	import { invokeMediaEdit } from './operations';
	import type { MediaCommandCatalogItem } from './generated/MediaCommandCatalogItem';
	import { useDocumentSelection } from '../selection.svelte';
	let {
		node,
		defaultChildren,
		disabled
	}: {
		node: UiNodeDto;
		defaultChildren?: Snippet<[String?, NodeInspectorChildFilter?]>;
		disabled: boolean;
	} = $props();
	const selection = useDocumentSelection();
	const session = $derived(appState.session);
	let catalog = $state<MediaCommandCatalogItem[]>([]),
		pending = $state(false),
		error = $state(''),
		page = $state(0);
	let disposed = false,
		request = 0;
	const commands = $derived(
		node.children.filter(
			(id) => session?.getNodeData(id)?.node_type === 'mapgyver.media.command.v1'
		)
	);
	const pages = $derived(Math.max(1, Math.ceil(commands.length / 32)));
	const currentPage = $derived(Math.min(page, pages - 1));
	const visible = $derived(new Set(commands.slice(currentPage * 32, (currentPage + 1) * 32)));
	const items = $derived<UiCreatableUserItem[]>(
		catalog.map((item) => ({
			node_type: item.kind,
			item_kind: 'mapgyver.media.command.v1',
			label: item.label,
			menu_path: [item.group],
			initial_params: [],
			select_when_created: false,
			separator_before: false
		}))
	);
	$effect(() => {
		const current = session,
			generation = current?.snapshotGeneration,
			uuid = node.uuid;
		const id = ++request;
		catalog = [];
		if (!current || current.status !== 'connected' || current.loading.visible) return;
		untrack(() => {
			void invokeMediaEdit(current, { kind: 'InspectCommandCatalog' })
				.then((result) => {
					if (
						disposed ||
						id !== request ||
						session !== current ||
						current.snapshotGeneration !== generation ||
						node.uuid !== uuid
					)
						return;
					if (result.kind === 'CommandCatalog') {
						catalog = result.commands;
						error = '';
					}
				})
				.catch((cause) => {
					if (!disposed && id === request) error = String(cause);
				});
		});
	});
	async function create(item: UiCreatableUserItem) {
		const kind = catalog.find((entry) => entry.kind === item.node_type)?.kind;
		const current = session,
			select = selection.capture();
		if (!current || !kind || pending || disabled) return;
		pending = true;
		error = '';
		try {
			const result = await invokeMediaEdit(current, {
				kind: 'CreateCommand',
				command_kind: kind,
				label: null
			});
			if (result.kind === 'CommandCreated') select(result.command);
		} catch (cause) {
			if (!disposed) error = String(cause);
		} finally {
			pending = false;
		}
	}
	onDestroy(() => {
		disposed = true;
		++request;
	});
</script>

<section class="media-commands-section" aria-label="Media commands">
	<header>
		<span>Commands</span><span class="spacer"></span>
		{#if pages > 1}
			<IconButton
				icon={arrowIcon}
				label="Previous commands"
				class="previous"
				disabled={disabled || currentPage === 0}
				onclick={() => {
					page = currentPage - 1;
				}} />
			<span>{currentPage + 1} / {pages}</span>
			<IconButton
				icon={arrowIcon}
				label="Next commands"
				disabled={disabled || currentPage + 1 === pages}
				onclick={() => {
					page = currentPage + 1;
				}} />
		{/if}
		<div title="Add Media Command">
			<NodeAddButton {node} {items} disabled={disabled || pending} onCreateItem={create} />
		</div>
	</header>
	{#if error}<p role="alert">{error}</p>{/if}
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
	.media-commands-section :global(.previous img) {
		transform: rotate(180deg);
	}
</style>
