<script lang="ts">
	import type { NodeInspectorComponentProps } from 'golden_ui';
	import ModuleIndicators from '$lib/components/modules/ModuleIndicators.svelte';

	let { node, defaultHeader, defaultChildren, collapsed }: NodeInspectorComponentProps = $props();

	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'folder';

	let showModuleIndicators = $derived(
		node.user_item_kind === MODULE_USER_ITEM_KIND && node.node_type !== MODULE_FOLDER_NODE_TYPE
	);
</script>

{#snippet moduleHeaderExtra()}
	{#if showModuleIndicators}
		<span class="module-header-indicators">
			<ModuleIndicators {node} />
		</span>
	{/if}
{/snippet}

{@render defaultHeader?.(moduleHeaderExtra)}

{#if collapsed !== true}
	<div class="node-inspector-content module-node-inspector">
		{@render defaultChildren?.()}
	</div>
{/if}

<style>
	.module-header-indicators {
		display: inline-flex;
		align-items: center;
		margin-left: 0.2rem;
	}

	.module-header-indicators :global(.module-indicators) {
		flex: 0 0 auto;
		gap: 0.1rem;
	}

	.module-header-indicators :global(.module-status-icon) {
		width: 1.15rem;
		height: 1.15rem;
	}

	.module-node-inspector {
		min-inline-size: 0;
	}
</style>
