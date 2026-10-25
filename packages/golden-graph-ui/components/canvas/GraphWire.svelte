<svelte:options namespace="svg" />

<script lang="ts">
	import type { GraphVisibleEdge } from '../../retained-edges';
	import type { GraphEdge } from '../../types';
	import { scaledWirePath } from '../../wire-geometry';
	let {
		view,
		remPx,
		selected,
		simplified = false,
		onSelect,
		onKeySelect
	}: {
		view: GraphVisibleEdge;
		remPx: number;
		selected: boolean;
		simplified?: boolean;
		onSelect: (event: PointerEvent, edge: GraphEdge) => void;
		onKeySelect: (event: KeyboardEvent, edge: GraphEdge) => void;
	} = $props();
	const gradientId = $props.id();
	let edge = $derived(view.edge);
	let path = $derived(scaledWirePath(view.wire, remPx));
	let usesGradient = $derived(
		edge.color !== undefined && edge.targetColor !== undefined && edge.color !== edge.targetColor
	);
	let stroke = $derived(usesGradient ? `url(#${gradientId})` : edge.color);
</script>

{#if path}
	{#if usesGradient}
		<defs
			><linearGradient
				id={gradientId}
				gradientUnits="userSpaceOnUse"
				x1={view.start.x * remPx}
				y1={view.start.y * remPx}
				x2={view.end.x * remPx}
				y2={view.end.y * remPx}
				><stop offset="0%" stop-color={edge.color} /><stop
					offset="100%"
					stop-color={edge.targetColor} /></linearGradient
			></defs>
	{/if}
	<path
		d={path}
		class="edge-hit"
		role="button"
		tabindex="0"
		aria-label="Select connection"
		aria-pressed={selected}
		vector-effect="non-scaling-stroke"
		onpointerdown={(event) => onSelect(event, edge)}
		onkeydown={(event) => onKeySelect(event, edge)} />
	<path
		d={path}
		class="edge"
		data-edge-id={edge.id}
		class:active={edge.active}
		class:gradient={usesGradient}
		class:invalid={edge.invalid}
		class:selected
		class:simplified
		style:--edge-color={stroke}
		vector-effect="non-scaling-stroke" />
{/if}

<style>
	path {
		fill: none;
	}

	path.edge {
		stroke: color-mix(
			in srgb,
			var(--edge-color, var(--ga-socket)) 68%,
			var(--gc-color-background, #11151b) 32%
		);
		stroke-width: 0.13rem;
		opacity: 0.72;
		pointer-events: none;
		transition:
			opacity 0.48s ease,
			stroke 0.48s ease,
			stroke-width 0.48s ease;
	}

	path.edge.gradient {
		stroke: var(--edge-color, var(--ga-socket));
	}

	path.edge.active {
		stroke: color-mix(in srgb, var(--edge-color, var(--ga-active)) 88%, white 12%);
		stroke-width: 0.15rem;
		opacity: 0.86;
		transition:
			opacity 0.06s ease-out,
			stroke 0.06s ease-out,
			stroke-width 0.06s ease-out;
	}

	path.edge.gradient.active {
		stroke: var(--edge-color, var(--ga-active));
	}

	path.edge.selected {
		stroke: var(--ga-selection);
		stroke-width: 0.25rem;
		opacity: 1;
	}

	path.edge.invalid {
		stroke: var(--ga-error);
		stroke-dasharray: 0.4rem 0.3rem;
	}

	path.edge.simplified {
		transition: none;
	}

	path.edge-hit {
		stroke: transparent;
		stroke-width: 0.85rem;
		pointer-events: stroke;
		cursor: pointer;
	}
</style>
