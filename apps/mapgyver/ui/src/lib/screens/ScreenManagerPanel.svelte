<script lang="ts">
	import {
		appState,
		ManagerListPanel,
		type PanelProps,
		type PanelState,
		type UiNodeDto
	} from 'golden_ui';
	import ScreenTreeActions from './ScreenTreeActions.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const isTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		!!node && ['mapgyver.screen', 'mapgyver.surface'].includes(node.node_type);
</script>

<section class="manager" aria-label={props.title ?? 'Screens'} data-panel-id={props.panelId}>
	<ManagerListPanel
		managerNodeType="mapgyver.screen.manager"
		searchPlaceholder="Search screens and surfaces…"
		emptyMessage="No screens"
		addButtonTitle="Create Screen"
		createDisabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession}
		{isTreeNode}
		canRenderNodeChildren={(node) => node.node_type === 'mapgyver.screen'}
		rowSupplementComponent={ScreenTreeActions} />
</section>

<style>
	.manager {
		container-type: inline-size;
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
