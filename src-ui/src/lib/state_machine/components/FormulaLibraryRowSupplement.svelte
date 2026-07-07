<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';

	let { node }: { node: UiNodeDto } = $props();

	const FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX = 'chataigne.formula.external.builtin:';
	const FORMULA_EXTERNAL_FILE_TAG = 'chataigne.formula.external.file';

	let builtIn = $derived(
		node.meta.tags.some((tag) => tag.startsWith(FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX))
	);
	let external = $derived(node.meta.tags.includes(FORMULA_EXTERNAL_FILE_TAG));
</script>

{#if builtIn}
	<span class="builtin-pill" title="Built-in formula">Built-in</span>
{:else if external}
	<span class="builtin-pill" title="External formula">External</span>
{/if}

<style>
	.builtin-pill {
		display: inline-flex;
		align-items: center;
		min-block-size: 1rem;
		padding: 0.08rem 0.28rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 75%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--gc-color-accent, #5d8cff) 12%, transparent);
		color: color-mix(in srgb, var(--gc-color-text) 66%, transparent);
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
	}
</style>
