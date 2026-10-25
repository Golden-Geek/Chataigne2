<script lang="ts">
	import type { FreeSurfaceOverview } from './free-surface-viewport.svelte';
	let {
		overview,
		activate
	}: { overview: FreeSurfaceOverview; activate(x: number, y: number): void } = $props();
	const pick = (event: MouseEvent) => {
		const bounds =
			event.currentTarget instanceof HTMLElement
				? event.currentTarget.getBoundingClientRect()
				: null;
		if (!bounds) return;
		// Enter/Space activates the center. Pointer activation uses exact authored hit testing.
		activate(
			event.detail === 0 ? bounds.left + bounds.width / 2 : event.clientX,
			event.detail === 0 ? bounds.top + bounds.height / 2 : event.clientY
		);
	};
</script>

<button
	type="button"
	class="dashboard-free-overview"
	aria-label="Dashboard overview. Select a widget to activate its controls, or zoom in."
	title="Groups show authored bounds. Select a widget or zoom in for its controls."
	style:left={`${overview.rect.left}px`}
	style:top={`${overview.rect.top}px`}
	style:width={`${overview.rect.width}px`}
	style:height={`${overview.rect.height}px`}
	onpointerdown={(event) => {
		if (event.button === 0) event.stopPropagation();
	}}
	onclick={pick}>
	<svg
		aria-hidden="true"
		viewBox={`0 0 ${overview.rect.width} ${overview.rect.height}`}
		preserveAspectRatio="none">
		{#each overview.groups as group}
			<rect
				x={group.rect.left - overview.rect.left}
				y={group.rect.top - overview.rect.top}
				width={group.rect.width}
				height={group.rect.height}
				data-group-members={group.count}
				vector-effect="non-scaling-stroke" />
		{/each}
	</svg>
	<span style:font-size={`${1 / overview.scale}rem`}>
		Overview · {overview.total.toLocaleString()} widgets in this surface
		<small>Select a widget or zoom in. Up to 16 selected controls are shown.</small>
	</span>
</button>

<style>
	.dashboard-free-overview {
		position: absolute;
		padding: 0;
		border: 0;
		border-radius: 0;
		background: transparent;
		color: var(--gc-color-text, #ddd);
		text-align: start;
		cursor: crosshair;
	}
	.dashboard-free-overview svg {
		position: absolute;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		overflow: hidden;
	}
	rect {
		fill: currentColor;
		fill-opacity: 0.08;
		stroke: currentColor;
		stroke-opacity: 0.3;
	}
	span {
		position: absolute;
		inset-block-start: 0;
		inset-inline-start: 0;
		max-inline-size: 32em;
		padding: 0.5em;
		background: var(--gc-color-background, #222);
		pointer-events: none;
	}
	small {
		display: block;
		font-size: 0.75em;
	}
</style>
