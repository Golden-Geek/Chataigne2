<script lang="ts">
	import {
		appState,
		FileDropFeedback,
		ManagerListPanel,
		type FileDropDestination,
		type PanelProps,
		type PanelState,
		type UiNodeDto
	} from 'golden_ui';
	import { isMediaTreeNode, canRenderMediaTreeChildren } from './tree';
	import MediaTreeActions from './MediaTreeActions.svelte';
	import { invokeMediaEdit } from './operations';
	let props: PanelProps = $props();
	let area: HTMLElement | undefined = $state();
	export const setPanelState = (_next: PanelState) => {};
	const selectedFolder = $derived.by(() => {
		const session = appState.session;
		const node =
			session?.selectedNodeId == null ? null : session.getNodeData(session.selectedNodeId);
		return node?.node_type === 'mapgyver.media.folder' ? node : null;
	});

	function fileDestination(element: Element): FileDropDestination | null {
		const session = appState.session;
		if (
			!area ||
			session?.status !== 'connected' ||
			session.loading.visible ||
			session.hasActiveEditSession
		)
			return null;
		const row = element.closest<HTMLElement>('.outliner-item-content[data-node-id]');
		const node = row ? session.getNodeData(Number(row.dataset.nodeId)) : null;
		const folder = node?.node_type === 'mapgyver.media.folder';
		const replace = node?.node_type === 'mapgyver.media.v11';
		if (row && !folder && !replace) return null;
		const generation = session.snapshotGeneration;
		return {
			bounds: row ?? area,
			label: replace
				? `Replace file in ${node.meta.label}`
				: folder
					? `Import into ${node.meta.label}`
					: 'Import Media',
			action: replace ? 'replace' : 'add',
			drop: async (paths) => {
				if (paths.length !== 1) throw new Error('Drop one file at a time.');
				if (appState.session !== session || session.snapshotGeneration !== generation) return;
				await invokeMediaEdit(session, {
					kind: 'DropMediaFile',
					path: paths[0],
					target: node?.uuid ?? null
				});
			}
		};
	}
</script>

<section
	class="manager"
	bind:this={area}
	aria-label={props.title ?? 'Medias'}
	data-panel-id={props.panelId}>
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
			['mapgyver.media.v11', 'mapgyver.media.reference.v4'].includes(node.node_type)}
		canRenderNodeChildren={canRenderMediaTreeChildren}
		canCreateNodeChildren={(node) => node.node_type === 'mapgyver.media.folder'}
		rowSupplementComponent={MediaTreeActions} />
	<FileDropFeedback {area} resolve={fileDestination} />
</section>

<style>
	.manager {
		position: relative;
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
