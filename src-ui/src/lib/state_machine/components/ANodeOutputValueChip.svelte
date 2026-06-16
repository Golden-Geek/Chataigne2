<script lang="ts">
	import type { FormulaOutputPreviewChip } from '../preview/formulaOutputPreviewStore.svelte';

	let {
		preview
	}: {
		preview: FormulaOutputPreviewChip | null;
	} = $props();
</script>

{#if preview}
	<span
		class="output-value-chip"
		class:error={preview.status === 'error'}
		class:stale={preview.status === 'stale' || preview.status === 'default_preview'}
		title={preview.title}>
		{preview.label}
	</span>
{/if}

<style>
	.output-value-chip {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		max-inline-size: 6.6rem;
		min-block-size: 1rem;
		padding: 0.06rem 0.32rem;
		border: 0.06rem solid color-mix(in srgb, var(--ga-active, #62d3ff) 55%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--ga-active, #62d3ff) 14%, var(--ga-node, #1d2430));
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 94%, white 6%);
		font-size: 0.58rem;
		font-weight: 700;
		line-height: 1.1;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		pointer-events: auto;
	}

	.output-value-chip.stale {
		border-color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 26%, transparent);
		background: color-mix(in srgb, var(--ga-node, #1d2430) 86%, transparent);
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 66%, transparent);
	}

	.output-value-chip.error {
		border-color: color-mix(in srgb, var(--ga-error, #ff5f75) 70%, transparent);
		background: color-mix(in srgb, var(--ga-error, #ff5f75) 16%, var(--ga-node, #1d2430));
		color: var(--ga-error, #ff5f75);
	}
</style>
