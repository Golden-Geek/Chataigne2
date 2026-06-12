<script lang="ts">
	import type { NodeInspectorComponentProps } from 'golden_ui';
	import ProcessorFormulaHeaderExtra from './ProcessorFormulaHeaderExtra.svelte';

	let { node, defaultHeader, defaultChildren, collapsed }: NodeInspectorComponentProps = $props();
</script>

{#snippet formulaHeaderExtra()}
	<ProcessorFormulaHeaderExtra {node} />
{/snippet}

{@render defaultHeader?.(formulaHeaderExtra)}

{#if collapsed !== true}
	<div class="node-inspector-content processor-formula-content">
		{@render defaultChildren?.()}
	</div>
{/if}

<style>
	.processor-formula-content {
		min-inline-size: 0;
	}
</style>
