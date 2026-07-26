<script lang="ts">
	import { showPanel, type NodeInspectorPanelHeaderComponentProps } from 'golden_ui';
	import ModuleIndicators from '$lib/components/modules/ModuleIndicators.svelte';
	import {
		moduleEditorPanelRequest,
		resolveModuleEditor
	} from '$lib/panels/modules/module-editor-registry';

	let { node, defaultHeader }: NodeInspectorPanelHeaderComponentProps = $props();

	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'module_folder';

	let showModuleIndicators = $derived(
		node.user_item_kind === MODULE_USER_ITEM_KIND && node.node_type !== MODULE_FOLDER_NODE_TYPE
	);
	let editorDescriptor = $derived(resolveModuleEditor(node));

	const openModuleEditor = (): void => {
		if (!editorDescriptor) return;
		showPanel(moduleEditorPanelRequest(editorDescriptor, node));
	};
</script>

{#snippet moduleHeaderExtra()}
	{#if showModuleIndicators}
		<ModuleIndicators {node} />
	{/if}
	{#if editorDescriptor}
		<button
			type="button"
			class="module-editor-open-btn"
			onclick={(event) => {
				event.stopPropagation();
				openModuleEditor();
			}}
			title={editorDescriptor.actionLabel}>
			<img src={editorDescriptor.iconUrl} alt="" class="module-editor-open-icon" />
			<span>{editorDescriptor.actionLabel}</span>
		</button>
	{/if}
{/snippet}

{@render defaultHeader?.(moduleHeaderExtra)}

<style>
	.module-editor-open-btn {
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

	.module-editor-open-btn:hover {
		background: color-mix(in srgb, var(--gc-color-accent) 18%, transparent);
	}

	.module-editor-open-icon {
		display: block;
		inline-size: 0.85rem;
		block-size: 0.85rem;
		flex: 0 0 auto;
	}
</style>
