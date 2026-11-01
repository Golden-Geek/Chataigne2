<script lang="ts">
	import { type UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import { mediaTreeItems, createMediaTreeItem } from './tree';
	import { useDocumentSelection } from '../selection.svelte';
	const selection = useDocumentSelection();
	let { node }: { node: UiNodeDto } = $props();
	let error = $state('');
</script>

{#if node.node_type === 'mapgyver.media.folder'}
	<NodeAddButton
		{node}
		items={mediaTreeItems}
		disabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession}
		onCreateItem={async (item) => {
			try {
				await createMediaTreeItem(node, item, selection.capture());
				error = '';
			} catch (cause) {
				error = String(cause);
			}
		}} />
{/if}
{#if error}<span role="alert" title={error}>!</span>{/if}
