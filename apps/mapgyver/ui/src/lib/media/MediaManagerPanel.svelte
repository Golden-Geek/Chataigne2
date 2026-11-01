<script lang="ts">
	import {
		appState,
		ManagerListPanel,
		type PanelProps,
		type PanelState,
		type UiNodeDto
	} from 'golden_ui';
	import { mediaTreeItems, mediaManagerItems, createMediaTreeItem, isMediaTreeNode } from './tree';
	import MediaTreeActions from './MediaTreeActions.svelte';
	import { useDocumentSelection } from '../selection.svelte';
	const selection = useDocumentSelection();
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	let error = $state('');
	const selectedFolder = $derived.by(() => {
		const session = appState.session;
		const node =
			session?.selectedNodeId == null ? null : session.getNodeData(session.selectedNodeId);
		return node?.node_type === 'mapgyver.media.folder' ? node : null;
	});
</script>

<section class="manager" aria-label={props.title ?? 'Media Manager'} data-panel-id={props.panelId}>
	{#if error}<p role="alert">{error}</p>{/if}
	<ManagerListPanel
		managerNodeType="mapgyver.media.manager"
		addTargetNode={selectedFolder}
		searchPlaceholder="Search media…"
		emptyMessage="No media"
		addButtonTitle="Create media or folder"
		addItems={selectedFolder ? mediaTreeItems : mediaManagerItems}
		createDisabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession}
		onCreateItem={async (parent, item) => {
			if (item.node_type === 'ImportPicture') {
				selection.select(parent.uuid);
				return;
			}
			try {
				await createMediaTreeItem(parent, item, selection.select);
				error = '';
			} catch (cause) {
				error = String(cause);
			}
		}}
		isTreeNode={isMediaTreeNode}
		canRenderNodeChildren={(node) =>
			node.node_type === 'mapgyver.media' || node.node_type === 'mapgyver.media.folder'}
		rowSupplementComponent={MediaTreeActions} />
</section>

<style>
	.manager {
		height: 100%;
		min-height: 0;
		display: flex;
		flex-direction: column;
	}
	.manager :global(.manager-list-panel) {
		flex: 1;
		min-height: 0;
	}
	p {
		margin: 0.5rem;
		color: var(--color-error, #e88);
	}
</style>
