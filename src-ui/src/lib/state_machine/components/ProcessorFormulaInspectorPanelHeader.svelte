<script lang="ts">
	import type { NodeInspectorPanelHeaderComponentProps } from 'golden_ui';
	import ProcessorFormulaHeaderExtra from './ProcessorFormulaHeaderExtra.svelte';

	let { node, defaultHeader }: NodeInspectorPanelHeaderComponentProps = $props();
</script>

{#snippet formulaHeaderExtra()}
	<ProcessorFormulaHeaderExtra {node} />
{/snippet}

{@render defaultHeader?.(formulaHeaderExtra)}
