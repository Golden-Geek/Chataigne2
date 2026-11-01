<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	let {
		icon,
		label,
		class: className = '',
		...rest
	}: HTMLButtonAttributes & { icon: string; label: string } = $props();
</script>

<button type="button" {...rest} class="gc-icon-button {className}" aria-label={label} title={label}>
	<img src={icon} alt="" draggable="false" />
</button>

<style>
	.gc-icon-button {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		flex: none;
		inline-size: 1.65rem;
		block-size: 1.65rem;
		padding: 0.2rem;
		border: 0;
		border-radius: 0.25rem;
		background: transparent;
		color: inherit;
		cursor: pointer;
	}
	.gc-icon-button:hover:not(:disabled) {
		background: var(--gc-control-hover-background, #ffffff18);
	}
	.gc-icon-button:focus-visible {
		outline: 0.1rem solid var(--gc-focus-color, #a3bfff);
		outline-offset: 0.05rem;
	}
	.gc-icon-button:disabled {
		opacity: 0.35;
		cursor: default;
	}
	img {
		inline-size: 100%;
		block-size: 100%;
		object-fit: contain;
	}
</style>
