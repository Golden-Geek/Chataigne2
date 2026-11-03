<script lang="ts">
	import {
		appState,
		ManagerListPanel,
		type PanelProps,
		type PanelState,
		type UiNodeDto
	} from 'golden_ui';
	import { isMediaTreeNode } from './tree';
	import MediaTreeActions from './MediaTreeActions.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const selectedFolder = $derived.by(() => {
		const session = appState.session;
		const node =
			session?.selectedNodeId == null ? null : session.getNodeData(session.selectedNodeId);
		return node?.node_type === 'mapgyver.media.folder' ? node : null;
	});
</script>

<section class="manager" aria-label={props.title ?? 'Media Manager'} data-panel-id={props.panelId}>
	<ManagerListPanel
		managerNodeType="mapgyver.media.manager"
		addTargetNode={selectedFolder}
		searchPlaceholder="Search media…"
		emptyMessage="No media"
		addButtonTitle="Create media or folder"
		createDisabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession}
		isTreeNode={isMediaTreeNode}
		nodeSourceDraggable={(node) =>
			['mapgyver.media.v5', 'mapgyver.media.reference.v1'].includes(node.node_type)}
		canRenderNodeChildren={(node) =>
			['mapgyver.media.folder', 'mapgyver.media.v5'].includes(node.node_type)}
		canCreateNodeChildren={(node) => node.node_type === 'mapgyver.media.folder'}
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
</style>
