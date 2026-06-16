<script lang="ts">
	import type { FormulaOutputPreviewChip } from '../preview/formulaOutputPreviewStore.svelte';

	let {
		preview
	}: {
		preview: FormulaOutputPreviewChip | null;
	} = $props();
</script>

{#if preview}
	{#if preview.value.kind === 'trigger'}
		<!-- Trigger outputs have no readout: the connection highlight already
		     signals that something was sent out. -->
	{:else if preview.value.kind === 'bool'}
		<span class="output-bool-readout" title={preview.title}>
			<input
				type="checkbox"
				checked={preview.value.value}
				readonly
				tabindex="-1"
				aria-label={preview.value.value ? 'Boolean output true' : 'Boolean output false'}
				onclick={(event) => event.preventDefault()} />
		</span>
	{:else}
		<span
			class="output-value-chip"
			class:active={preview.active}
			class:error={preview.status === 'error'}
			class:inactive={preview.status === 'stale' ||
				preview.status === 'suppressed' ||
				preview.status === 'unavailable'}
			title={preview.title}>
			{preview.label}
		</span>
	{/if}
{/if}

<style>
	.output-value-chip,
	.output-bool-readout {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		min-block-size: 1rem;
		box-sizing: border-box;
		pointer-events: auto;
		opacity: 0.78;
		transition:
			opacity 0.05s ease-out,
			filter 0.05s ease-out,
			border-color 0.05s ease-out,
			background-color 0.05s ease-out,
			color 0.05s ease-out;
	}

	.output-value-chip {
		max-inline-size: 6.6rem;
		padding: 0.06rem 0.32rem;
		border: 0.06rem solid color-mix(in srgb, var(--ga-active, #62d3ff) 38%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--ga-active, #62d3ff) 8%, var(--ga-node, #1d2430));
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 76%, transparent);
		font-size: 0.58rem;
		font-weight: 700;
		line-height: 1.1;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.output-value-chip.active {
		border-color: color-mix(in srgb, var(--ga-active, #62d3ff) 62%, transparent);
		background: color-mix(in srgb, var(--ga-active, #62d3ff) 14%, var(--ga-node, #1d2430));
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 94%, white 6%);
		filter: brightness(1.06);
		opacity: 1;
		transition:
			opacity 0.05s ease-out,
			filter 0.05s ease-out,
			border-color 0.05s ease-out,
			background-color 0.05s ease-out,
			color 0.05s ease-out;
	}

	.output-value-chip.inactive {
		opacity: 0.62;
	}

	.output-value-chip.error {
		border-color: color-mix(in srgb, var(--ga-error, #ff5f75) 70%, transparent);
		background: color-mix(in srgb, var(--ga-error, #ff5f75) 16%, var(--ga-node, #1d2430));
		color: var(--ga-error, #ff5f75);
		opacity: 1;
	}

	/* Bool output: just a read-only checkbox, no box decoration and no
	   animation so fast on/off toggles stay visible. */
	.output-bool-readout {
		justify-content: center;
		inline-size: 1rem;
		block-size: 1rem;
		opacity: 1;
		transition: none;
	}

	.output-bool-readout input {
		inline-size: 0.92rem;
		block-size: 0.92rem;
		margin: 0;
		accent-color: var(--ga-active, #62d3ff);
		pointer-events: none;
		transition: none;
	}
</style>
