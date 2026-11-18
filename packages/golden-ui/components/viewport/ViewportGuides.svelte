<script lang="ts">
	import type { AlignmentGuide } from './alignment';
	let {
		guides,
		width,
		height,
		native = false
	}: { guides: AlignmentGuide[]; width: number; height: number; native?: boolean } = $props();
</script>

{#if guides.length}
	<svg
		viewBox={`0 0 ${width} ${height}`}
		preserveAspectRatio="none"
		aria-hidden="true"
		data-alignment-guides
		data-native-overlay-control={native ? '' : undefined}>
		{#each guides as guide (guide.axis)}
			<line
				x1={guide.axis === 'x' ? guide.value : 0}
				x2={guide.axis === 'x' ? guide.value : width}
				y1={guide.axis === 'y' ? guide.value : 0}
				y2={guide.axis === 'y' ? guide.value : height} />
		{/each}
	</svg>
{/if}

<style>
	svg {
		position: absolute;
		inset: 0;
		width: 100%;
		height: 100%;
		overflow: hidden;
		pointer-events: none;
	}
	line {
		stroke: var(--gc-color-focus);
		stroke-opacity: 0.8;
		stroke-width: 0.0625rem;
		stroke-dasharray: 0.25rem 0.2rem;
		vector-effect: non-scaling-stroke;
	}
</style>
