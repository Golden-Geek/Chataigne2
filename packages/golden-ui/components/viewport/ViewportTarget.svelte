<script lang="ts">
	import type { HTMLButtonAttributes } from 'svelte/elements';
	import { viewportContour } from './contour';
	let {
		x,
		y,
		label,
		identity,
		enabled = true,
		...attributes
	}: HTMLButtonAttributes & {
		x: number;
		y: number;
		label: string;
		identity?: string;
		enabled?: boolean;
	} = $props();
</script>

<!-- A selection target at a known pivot does not invent unavailable object bounds. -->
<button
	{...attributes}
	type="button"
	class="viewport-target"
	data-viewport-selection-target
	use:viewportContour={{ identity, enabled }}
	style:left={`${x * 100}%`}
	style:top={`${y * 100}%`}
	aria-label={label}></button>

<style>
	.viewport-target {
		position: absolute;
		width: 1rem;
		height: 1rem;
		box-sizing: border-box;
		padding: 0;
		transform: translate(-50%, -50%);
		background: transparent;
		cursor: pointer;
		pointer-events: auto;
		touch-action: none;
	}
</style>
