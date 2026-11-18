<script lang="ts">
	import { appState } from 'golden_ui';
	import { getContainerColorForNode } from 'golden_ui/store/node-types';
	import type { MatrixGroupSpan } from '../media/generated/MatrixGroupSpan';
	import { gridSize, type Metrics, type projectGrid } from './viewport';
	import { groupRail } from './group-rails';
	let {
		spans,
		projection,
		metrics
	}: {
		spans: MatrixGroupSpan[];
		projection: ReturnType<typeof projectGrid>;
		metrics: Metrics;
	} = $props();
	const session = $derived(appState.session);
	const height = $derived(metrics.height / Math.max(1, metrics.rem));
	const width = $derived(Math.min(gridSize.label, metrics.width / Math.max(1, metrics.rem)));
	const depth = $derived(Math.max(0, ...spans.map((span) => span.depth)));
	const rails = $derived(
		spans.map((span) => ({ span, geometry: groupRail(span, projection, height, depth) }))
	);
	function color(id: string) {
		const node = session?.graph.state.nodesById.getByUuid(id);
		return node?.meta.presentation?.color
			? getContainerColorForNode(node)
			: 'var(--gc-color-accent)';
	}
	function enabled(id: string) {
		const node = session?.graph.state.nodesById.getByUuid(id);
		return node ? session?.isNodeEnabledInHierarchy(node.node_id) === true : true;
	}
</script>

{#if height > projection.header && width > 0}
	<svg
		class="group-rails"
		aria-hidden="true"
		focusable="false"
		data-matrix-group-rails
		viewBox={`0 0 ${width} ${height}`}
		style:width={`${width}rem`}
		style:height={`${metrics.height}px`}>
		{#each rails as { span, geometry } (span.id)}
			{#if geometry}
				<path
					class="wash"
					d={geometry.fill}
					style:fill={color(span.id)}
					data-enabled={enabled(span.id)} />
				<path
					d={geometry.path}
					data-group-rail={span.id}
					data-depth={span.depth}
					data-start-clipped={geometry.startClipped}
					data-end-clipped={geometry.endClipped}
					data-enabled={enabled(span.id)}
					style:--group-color={color(span.id)} />
			{/if}
		{/each}
	</svg>
{/if}

<style>
	.group-rails {
		position: absolute;
		/* Keep decorations outside the scrollport's extent, like native Cell previews. */
		inset: 0 auto auto 0;
		/* Row cards and their selection contours paint above their enclosing container. */
		z-index: 1;
		pointer-events: none;
		overflow: hidden;
	}
	path {
		fill: none;
		stroke: var(--group-color);
		stroke-opacity: 0.55;
		stroke-width: 0.08;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
	.wash {
		stroke: none;
		opacity: 0.06;
	}
	path[data-enabled='false'] {
		opacity: 0.45;
	}
	.wash[data-enabled='false'] {
		opacity: 0.025;
	}
</style>
