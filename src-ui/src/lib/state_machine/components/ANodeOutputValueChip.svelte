<script lang="ts">
	import parameterTriggerIcon from '../../golden_ui/style/icons/parameter/trigger.svg';
	import type { FormulaOutputPreviewChip } from '../preview/formulaOutputPreviewStore.svelte';

	let {
		preview
	}: {
		preview: FormulaOutputPreviewChip | null;
	} = $props();
</script>

{#if preview}
	{#if preview.value.kind === 'trigger'}
		<span
			class="output-trigger-readout"
			class:active={preview.active}
			class:error={preview.status === 'error'}
			class:inactive={preview.status === 'stale' ||
				preview.status === 'suppressed' ||
				preview.status === 'unavailable'}
			title={preview.title}
			aria-label={preview.active ? 'Trigger output active' : 'Trigger output idle'}>
			<img src={parameterTriggerIcon} alt="" aria-hidden="true" />
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
	.output-trigger-readout {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		min-block-size: 1rem;
		box-sizing: border-box;
		pointer-events: auto;
		opacity: 0.78;
		transition:
			opacity 0.46s ease,
			filter 0.46s ease,
			border-color 0.46s ease,
			background-color 0.46s ease,
			color 0.46s ease;
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
			opacity 0.06s ease-out,
			filter 0.06s ease-out,
			border-color 0.06s ease-out,
			background-color 0.06s ease-out,
			color 0.06s ease-out;
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

	.output-trigger-readout {
		justify-content: center;
		inline-size: 2.15rem;
		block-size: 1rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-trigger, #8b64ff) 44%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-trigger, #8b64ff) 54%, var(--ga-node, #1d2430));
		filter: brightness(0.92) saturate(0.9);
	}

	.output-trigger-readout.active {
		border-color: color-mix(
			in srgb,
			var(--gc-color-trigger-engine, var(--gc-color-trigger-on, var(--ga-active, #f2c01a))) 72%,
			transparent
		);
		background: color-mix(
			in srgb,
			var(--gc-color-trigger-engine, var(--gc-color-trigger-on, var(--ga-active, #f2c01a))) 82%,
			var(--ga-node, #1d2430)
		);
		filter: brightness(1.08) saturate(1.08);
		opacity: 1;
		transition:
			opacity 0.06s ease-out,
			filter 0.06s ease-out,
			border-color 0.06s ease-out,
			background-color 0.06s ease-out;
	}

	.output-trigger-readout.inactive {
		opacity: 0.58;
	}

	.output-trigger-readout.error {
		border-color: color-mix(in srgb, var(--ga-error, #ff5f75) 70%, transparent);
		background: color-mix(in srgb, var(--ga-error, #ff5f75) 22%, var(--ga-node, #1d2430));
		opacity: 1;
	}

	.output-trigger-readout img {
		inline-size: 0.72rem;
		block-size: 0.72rem;
		padding: 0.1rem;
		filter: brightness(1.12);
	}
</style>
