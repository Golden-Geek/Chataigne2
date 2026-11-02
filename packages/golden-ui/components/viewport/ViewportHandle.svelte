<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	let {
		x,
		y,
		label,
		ghost = false,
		...attributes
	}: HTMLButtonAttributes & {
		x: number;
		y: number;
		label: string;
		ghost?: boolean;
	} = $props();
</script>

<button
	{...attributes}
	type="button"
	class="viewport-handle"
	class:ghost
	style:left={`${x * 100}%`}
	style:top={`${y * 100}%`}
	aria-label={label}
	aria-hidden={ghost ? 'true' : undefined}
	tabindex={-1}
	disabled={ghost || attributes.disabled}></button>

<style>
	.viewport-handle {
		position: absolute;
		width: 0.85rem;
		height: 0.85rem;
		box-sizing: border-box;
		padding: 0;
		transform: translate(-50%, -50%);
		border: 0.1rem solid #467bcc;
		border-radius: 50%;
		background: #dceaff;
		cursor: crosshair;
		pointer-events: auto;
		touch-action: none;
	}
	.viewport-handle:disabled {
		cursor: default;
	}
	.ghost {
		background: transparent;
		border-color: #ffe19b;
		pointer-events: none;
	}
</style>
