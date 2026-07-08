<script lang="ts">
	import type {
		FormulaPreviewEditLevel,
		FormulaPreviewSessionModel
	} from '../preview/formulaPreviewSessionStore.svelte';

	let { model }: { model: FormulaPreviewSessionModel } = $props();

	const modes: { id: FormulaPreviewEditLevel; label: string; color: string }[] = [
		{ id: 'formula_recipe', label: 'Defaults', color: 'var(--gc-color-readonly)' },
		{ id: 'processor_instance', label: 'Instance', color: 'var(--gc-color-accent)' },
		{ id: 'selected_lane', label: 'Lane', color: 'var(--gc-color-accent)' }
	];
</script>

<div class="preview-context" aria-label="Formula preview context">
	<!-- <div class="preview-labels">
		<span class="preview-title">{model.title}</span>
		<span class="preview-subtitle">{model.subtitle}</span>
	</div> -->
	<div class="preview-mode" aria-label="Formula preview mode">
		{#each modes as mode}
			<span
				class="preview-mode-item"
				class:active={model.level === mode.id}
				class:disabled={mode.id !== 'formula_recipe' && model.processorNodeId === null}
				style="--mode-color: {mode.color};"
				aria-current={model.level === mode.id ? 'true' : undefined}>
				{mode.label}
			</span>
		{/each}
	</div>
</div>

<style>
	.preview-context {
		display: inline-flex;
		align-items: center;
		gap: 0.55rem;
		min-inline-size: 0;
	}

	/* .preview-labels {
		display: inline-flex;
		flex-direction: column;
		justify-content: center;
		min-inline-size: 0;
		max-inline-size: 18rem;
		line-height: 1.05;
	}

	.preview-title,
	.preview-subtitle {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.preview-title {
		color: var(--gc-color-text);
		font-size: 0.72rem;
		font-weight: 700;
	}

	.preview-subtitle {
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.62rem;
		font-weight: 600;
	} */

	.preview-mode {
		display: inline-grid;
		grid-template-columns: repeat(3, minmax(0, auto));
		align-items: center;
		min-block-size: 1.45rem;
		overflow: hidden;
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--gc-color-background) 80%, transparent);
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
		border-radius:1rem;
		white-space: nowrap;
		border-inline-start: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 35%, transparent);
	}

	.preview-mode-item:first-child {
		border-inline-start: none;
	}

	.preview-mode-item.active {
		color: var(--gc-color-text);
		background: color-mix(in srgb, var(--mode-color) 60%, transparent);
	}

	.preview-mode-item.disabled {
		color: color-mix(in srgb, var(--gc-color-text) 34%, transparent);
	}
</style>
