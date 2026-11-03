<script lang="ts">
	import {
		appState,
		ManagerListPanel,
		type PanelProps,
		type PanelState,
		type UiNodeDto,
		type UiCreatableUserItem
	} from 'golden_ui';
	import { invokeMediaEdit } from '../media/operations';
	import { useDocumentSelection } from '../selection.svelte';
	let props: PanelProps = $props();
	export const setPanelState = (_next: PanelState) => {};
	let error = $state('');
	const items: UiCreatableUserItem[] = [
		{
			node_type: 'mapgyver.app_control.v1',
			item_kind: 'application',
			label: 'Application',
			menu_path: [],
			initial_params: [],
			select_when_created: true
		}
	];
	const isTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		!!node && node.node_type === 'mapgyver.app_control.v1';
	async function create() {
		const session = appState.session;
		if (!session) return;
		const selectCreated = selection.capture();
		try {
			const result = await invokeMediaEdit(session, {
				kind: 'Applications',
				edit: { kind: 'Create', label: null }
			});
			if (result.kind === 'ApplicationCreated') selectCreated(result.application);
			error = '';
		} catch (cause) {
			error = String(cause);
		}
	}
	const selection = useDocumentSelection();
</script>

<section class="manager" aria-label={props.title ?? 'Applications'} data-panel-id={props.panelId}>
	{#if error}<p role="alert">{error}</p>{/if}
	<ManagerListPanel
		managerNodeType="mapgyver.media.manager"
		searchPlaceholder="Search applications…"
		emptyMessage="No applications"
		addButtonTitle="Create application"
		addItems={items}
		createDisabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession}
		onCreateItem={create}
		{isTreeNode} />
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
	}
</style>
