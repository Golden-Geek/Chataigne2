<script lang="ts">
	let {
		valid,
		showLabel = true,
		title = valid ? 'Valid' : 'Invalid'
	}: {
		valid: boolean;
		showLabel?: boolean;
		title?: string;
	} = $props();
</script>

<span class="validation-chip" class:valid class:icon-only={!showLabel} {title} aria-label={title}>
	{#if showLabel}
		<span class="validation-chip-label">{valid ? 'Valid' : 'Invalid'}</span>
	{:else}
		<!-- <span class="validation-chip-dot" aria-hidden="true"></span> -->
	{/if}
</span>

<style>
	.validation-chip {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		gap: 0.28rem;
		block-size: 1.2rem;
		min-inline-size: 3.4rem;
		padding-inline: 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 70%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--gc-color-text) 10%, transparent);
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.68rem;
		font-weight: 700;
		line-height: 1;
		white-space: nowrap;
	}

	.validation-chip.valid {
		border-color: color-mix(in srgb, var(--gc-success, #47a66a) 42%, transparent);
		background: color-mix(in srgb, var(--gc-success, #47a66a) 18%, transparent);
		color: var(--gc-success, #47a66a);
	}

	.validation-chip.icon-only {
		height: 0.6rem;
		inline-size: 1.85rem;
		min-inline-size: 1.85rem;
		padding-inline: 0;
	}

	.validation-chip.icon-only.valid {
		background: color-mix(in srgb, var(--gc-success, #47a66a) 80%, transparent);
		box-shadow: inset 0 0 0.5rem
			color-mix(in srgb, var(--gc-success, #47a66a) 30%, rgba(100, 255, 0, 0.8));
	}

	/* .validation-chip-dot {
		flex: 0 0 auto;
		inline-size: 80%;
		block-size: 60%;
		border-radius: 999rem;
		background: currentColor;
	} */

	.validation-chip-label {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
