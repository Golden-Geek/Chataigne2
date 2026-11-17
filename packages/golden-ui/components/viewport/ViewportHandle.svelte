<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { viewportHandleAppearance } from './feedback';
	import './feedback.css';
	let {
		x,
		y,
		label,
		ghost = false,
		selected,
		paint = true,
		...attributes
	}: HTMLButtonAttributes & {
		x: number;
		y: number;
		label: string;
		ghost?: boolean;
		selected?: boolean;
		paint?: boolean;
	} = $props();
	const appearance = $derived(viewportHandleAppearance(selected === true, ghost));
</script>

<button
	{...attributes}
	type="button"
	class="viewport-handle"
	class:ghost
	class:selected={selected === true}
	class:unpainted={!paint}
	data-viewport-handle
	style:background={paint ? `#${appearance.fill}` : 'transparent'}
	style:border-color={paint ? `#${appearance.stroke}` : 'transparent'}
	style:--gc-viewport-handle-halo={`#${appearance.halo}`}
	style:left={`${x * 100}%`}
	style:top={`${y * 100}%`}
	aria-label={label}
	aria-pressed={selected ?? attributes['aria-pressed']}
	aria-hidden={ghost ? 'true' : undefined}
	tabindex={attributes.tabindex ?? -1}
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
	.viewport-handle:disabled,
	.viewport-handle[aria-disabled='true'] {
		cursor: default;
	}
	.ghost {
		background: transparent;
		border-color: #ffe19b;
		pointer-events: none;
	}
	.selected {
		box-shadow: 0 0 0 0.0625rem var(--gc-viewport-handle-halo);
	}
	.viewport-handle:focus-visible {
		outline: 0.125rem solid var(--gc-focus-color, #a3bfff);
		outline-offset: 0.125rem;
	}
</style>
