<script lang="ts">
	import type { NodeId, NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		soundCardAncestorByType,
		soundCardDirectionConfigured
	} from './sound-card-routing-model';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let nodes = $derived(session?.graph.state.nodesById ?? new Map<NodeId, UiNodeDto>());
	let parentById = $derived(session?.graph.state.parentById ?? new Map<NodeId, NodeId>());
	let module = $derived(soundCardAncestorByType(nodes, parentById, node, 'sound_card_module'));
	let direction = $derived(
		node.node_type === 'sound_card_input_parameters'
			? 'input'
			: node.node_type === 'sound_card_output_parameters'
				? 'output'
				: null
	);
	let visible = $derived(
		direction === 'input'
			? soundCardDirectionConfigured(nodes, module, 'input')
			: direction === 'output'
				? soundCardDirectionConfigured(nodes, module, 'output')
				: false
	);
</script>

{#if visible}
	{@render defaultHeader?.()}

	{#snippet parameterContent()}
		{@render defaultChildren?.()}
	{/snippet}

	{@render defaultContent?.(parameterContent, 'sound-card-direction-parameters')}
{/if}
