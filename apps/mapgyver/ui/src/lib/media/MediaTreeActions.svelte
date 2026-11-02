<script lang="ts">
	import { appState, type UiNodeDto } from 'golden_ui';
	import NodeAddButton from 'golden_ui/components/common/NodeAddButton.svelte';
	let { node }: { node: UiNodeDto } = $props();
</script>

{#if node.node_type === 'mapgyver.media.folder' || node.node_type === 'mapgyver.media'}
	<NodeAddButton
		{node}
		disabled={appState.session?.status !== 'connected' ||
			appState.session.loading.visible ||
			appState.session.hasActiveEditSession} />
{/if}
