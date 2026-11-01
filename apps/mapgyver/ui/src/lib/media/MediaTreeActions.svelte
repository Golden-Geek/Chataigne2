<script lang="ts">
	import { IconButton, type UiNodeDto } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import { mediaTreeItems, createMediaTreeItem } from './tree';
	import { invokeMediaEdit } from './operations';
	import { appState } from 'golden_ui';
	import { useDocumentSelection } from '../selection.svelte';
	const selection = useDocumentSelection();
	let { node }: { node: UiNodeDto } = $props();
	let error = $state('');
</script>

{#if node.node_type === 'mapgyver.media.folder'}
	<NodeAddButton
		{node}
		items={mediaTreeItems}
		onCreateItem={async (item) => {
			try {
				await createMediaTreeItem(node, item, selection.select);
				error = '';
			} catch (cause) {
				error = String(cause);
			}
		}} />
{:else if node.node_type === 'mapgyver.media'}
	<IconButton
		icon={addIcon}
		label="Add channel"
		onclick={async (event) => {
			event.stopPropagation();
			const session = appState.session;
			if (!session) return;
			try {
				await invokeMediaEdit(session, { kind: 'AddChannel', media: node.uuid, label: null });
				error = '';
			} catch (cause) {
				error = String(cause);
			}
		}} />
{/if}
{#if error}<span role="alert" title={error}>!</span>{/if}
