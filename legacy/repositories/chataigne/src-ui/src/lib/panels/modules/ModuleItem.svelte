<script lang="ts">
	import ModuleIndicators from '$lib/components/modules/ModuleIndicators.svelte';
	import type { UiNodeDto } from '$lib/golden_ui/types';
	import { appState } from '$lib/golden_ui/store/app-state.svelte';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	const MODULE_USER_ITEM_KIND = 'module';
	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(graph?.nodesById.get(node.node_id) ?? node);
	let isModuleNode = $derived(liveNode.user_item_kind === MODULE_USER_ITEM_KIND);
</script>

{#if isModuleNode}
	<ModuleIndicators {node} />
{/if}
