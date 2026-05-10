<script lang="ts">
	import type { NodeInspectorPanelHeaderComponentProps } from 'golden_ui';
	import ModuleIndicators from '$lib/components/modules/ModuleIndicators.svelte';

	let { node, defaultHeader }: NodeInspectorPanelHeaderComponentProps = $props();

	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'folder';

	let showModuleIndicators = $derived(
		node.user_item_kind === MODULE_USER_ITEM_KIND && node.node_type !== MODULE_FOLDER_NODE_TYPE
	);
</script>

{#snippet moduleHeaderExtra()}
	{#if showModuleIndicators}
		<ModuleIndicators {node} />
	{/if}
{/snippet}

{@render defaultHeader?.(moduleHeaderExtra)}

<style>
</style>
