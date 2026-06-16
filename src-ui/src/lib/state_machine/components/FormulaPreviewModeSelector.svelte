<script lang="ts">
	import type {
		FormulaPreviewEditLevel,
		FormulaPreviewSessionModel
	} from '../preview/formulaPreviewSessionStore.svelte';

	let { model }: { model: FormulaPreviewSessionModel } = $props();

	const modes: { id: FormulaPreviewEditLevel; label: string }[] = [
		{ id: 'formula_recipe', label: 'Recipe' },
		{ id: 'processor_instance', label: 'Instance' },
		{ id: 'selected_lane', label: 'Lane' }
	];
</script>

<div class="preview-mode" aria-label="Formula preview mode">
	{#each modes as mode}
		<span
			class="preview-mode-item"
			class:active={model.level === mode.id}
			class:disabled={mode.id !== 'formula_recipe' && model.processorNodeId === null}
			aria-current={model.level === mode.id ? 'true' : undefined}>
			{mode.label}
		</span>
	{/each}
</div>

<style>
	.preview-mode {
		display: inline-grid;
		grid-template-columns: repeat(3, minmax(0, auto));
		align-items: center;
		min-block-size: 1.45rem;
		overflow: hidden;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 80%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-background) 92%, transparent);
	}

	.preview-mode-item {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-inline-size: 3.4rem;
		padding: 0.2rem 0.45rem;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.68rem;
		font-weight: 650;
		letter-spacing: 0;
		line-height: 1;
		white-space: nowrap;
		border-inline-start: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}

	.preview-mode-item:first-child {
		border-inline-start: none;
	}

	.preview-mode-item.active {
		color: var(--gc-color-background);
		background: var(--gc-color-accent);
	}

	.preview-mode-item.disabled {
		color: color-mix(in srgb, var(--gc-color-text) 34%, transparent);
	}
</style>
