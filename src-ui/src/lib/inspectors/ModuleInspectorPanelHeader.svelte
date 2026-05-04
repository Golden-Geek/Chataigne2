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
		<span class="inspector-header-module-indicators">
			<ModuleIndicators {node} />
		</span>
	{/if}
{/snippet}

{@render defaultHeader?.(moduleHeaderExtra)}

<style>
	.inspector-header-module-indicators {
		display: inline-flex;
		align-items: center;
		margin-left: 0.2rem;
		min-width: 0;
	}

	.inspector-header-module-indicators :global(.module-indicators) {
		flex: 0 0 auto;
		gap: 0.1rem;
	}

	.inspector-header-module-indicators :global(.module-status-icon) {
		width: 1.15rem;
		height: 1.15rem;
	}
</style>