<script lang="ts">
	import NodeInspector from './NodeInspector.svelte';
	import type { GraphState } from '../store/graph';
	import type { UiClient, UiEditIntent } from '../types';

	let {
		state,
		client,
		onIntent
	}: {
		state: GraphState;
		client: UiClient;
		onIntent: (intent: UiEditIntent) => void;
	} = $props();

	const selectedFallbackNode = $derived(
		state.selectedNodeId !== null ? state.nodesById.get(state.selectedNodeId) ?? null : null
	);
</script>

<NodeInspector
	{client}
	selectedNodeId={state.selectedNodeId}
	fallbackNode={selectedFallbackNode}
	{onIntent}
/>
