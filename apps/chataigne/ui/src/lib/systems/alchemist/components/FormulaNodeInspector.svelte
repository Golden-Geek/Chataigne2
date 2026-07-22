<script lang="ts">
	import type { NodeInspectorComponentProps } from 'golden_ui';

	let { defaultHeader, defaultContent }: NodeInspectorComponentProps = $props();
</script>

{@render defaultHeader?.()}

{#snippet formulaContent()}
	<p class="formula-inspector-summary">
		Edit this Formula in the Alchemist graph. Graph nodes are intentionally omitted from the generic
		Inspector.
	</p>
{/snippet}

{@render defaultContent?.(formulaContent, 'formula-node-inspector')}

<style>
	.formula-inspector-summary {
		margin: 0;
		padding: 0.55rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 75%, transparent);
		border-radius: 0.4rem;
		background: rgb(from var(--gc-color-text) r g b / 4%);
		color: rgb(from var(--gc-color-text) r g b / 70%);
		font-size: 0.72rem;
		line-height: 1.45;
	}
</style>
