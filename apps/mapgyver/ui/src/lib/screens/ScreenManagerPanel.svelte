<script lang="ts">
	import { onMount } from 'svelte';
	import { ManagerListPanel, type PanelProps, type PanelState, type UiNodeDto } from 'golden_ui';
	import { useScreenAuthoring } from './context.svelte';
	import ScreenTreeActions from './ScreenTreeActions.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	const owner = useScreenAuthoring();
	onMount(() => owner.attach());
	const isTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		!!node && ['mapgyver.screen', 'mapgyver.surface'].includes(node.node_type);
</script>

<section class="manager" aria-label={props.title ?? 'Screen Manager'} data-panel-id={props.panelId}>
	{#if owner.view.error}<p role="alert">{owner.view.error}</p>{/if}
	<ManagerListPanel
		managerNodeType="mapgyver.screen.manager"
		searchPlaceholder="Search screens and surfaces…"
		emptyMessage="No screens"
		addButtonTitle="Create Screen"
		createDisabled={owner.controller.busy || owner.session?.hasActiveEditSession === true}
		{isTreeNode}
		canRenderNodeChildren={(node) => node.node_type === 'mapgyver.screen'}
		rowSupplementComponent={ScreenTreeActions} />
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
