<script lang="ts">
	import { showPanel, type NodeInspectorPanelHeaderComponentProps } from 'golden_ui';
	import generatorsIconUrl from '$lib/assets/icons/module/generators.svg';
	import ModuleIndicators from '$lib/components/modules/ModuleIndicators.svelte';

	let { node, defaultHeader }: NodeInspectorPanelHeaderComponentProps = $props();

	const SPATIALIZER_MODULE_TYPE = 'spatializer_module';
	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'module_folder';

	let showModuleIndicators = $derived(
		node.user_item_kind === MODULE_USER_ITEM_KIND && node.node_type !== MODULE_FOLDER_NODE_TYPE
	);
	let showSpatializerEditor = $derived(node.node_type === SPATIALIZER_MODULE_TYPE);

	const openSpatializerEditor = (): void => {
		showPanel({
			panelId: 'spatializer-editor',
			panelType: 'spatializerEditor',
			title: `Spatializer: ${node.meta.label}`,
			params: { moduleNodeId: node.node_id },
			position: {
				referencePanelId: 'state-machine',
				direction: 'within'
			}
		});
	};
</script>

{#snippet moduleHeaderExtra()}
	{#if showModuleIndicators}
		<ModuleIndicators {node} />
	{/if}
	{#if showSpatializerEditor}
		<button
			type="button"
			class="spatializer-open-btn"
			onclick={(event) => {
				event.stopPropagation();
				openSpatializerEditor();
			}}
			title="Edit Spatializer">
			<img src={generatorsIconUrl} alt="" class="spatializer-open-icon" />
			<span>Edit Spatializer</span>
		</button>
	{/if}
{/snippet}

{@render defaultHeader?.(moduleHeaderExtra)}

<style>
	.spatializer-open-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.28rem;
		flex: 0 0 auto;
		margin-inline-start: 0.2rem;
		padding: 0.12rem 0.28rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.65rem;
		cursor: pointer;
		overflow: hidden;
		transition: background 0.12s ease;
	}

	.spatializer-open-btn:hover {
		background: color-mix(in srgb, var(--gc-color-accent) 18%, transparent);
	}

	.spatializer-open-icon {
		display: block;
		inline-size: 0.85rem;
		block-size: 0.85rem;
		flex: 0 0 auto;
	}
</style>
