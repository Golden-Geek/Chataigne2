<script lang="ts">
	import { fade } from 'svelte/transition';
	import type { GraphWorldContentContext } from 'golden_graph_ui';
	import type { SpatialEndpoint } from './types';
	import type { SpatializerGeometry } from './geometry.svelte';
	import type { SpatializerInteraction } from './interaction.svelte';
	import { MODE_VORONOI, HIGHLIGHT_FADE_MS } from './constants';

	let {
		world,
		geometry,
		interaction,
		targets,
		sources,
		spatializerMode,
		debugView,
		selectedEndpoint,
		isSelected,
		svg = $bindable(null)
	}: {
		world: GraphWorldContentContext;
		geometry: SpatializerGeometry;
		interaction: SpatializerInteraction;
		targets: SpatialEndpoint[];
		sources: SpatialEndpoint[];
		spatializerMode: string;
		debugView: boolean;
		selectedEndpoint: SpatialEndpoint | null;
		isSelected(endpoint: SpatialEndpoint): boolean;
		svg?: SVGSVGElement | null;
	} = $props();
</script>

<svg
	bind:this={svg}
	class="spatializer-map"
	role="application"
	aria-label="Spatializer map"
	onpointermove={interaction.handleWorldPointerMove}
	onpointerup={interaction.finishWorldPointer}
	onpointercancel={interaction.cancelWorldPointer}>
	{#each geometry.voronoiCellsForBounds(geometry.voronoiViewBounds(world)) as cell (cell.key)}
		<polygon
			class="voronoi-cell"
			class:highlighted={geometry.highlightedVoronoiCell(cell)}
			points={geometry.polygonPoints(cell.points, world)}
			fill={cell.color}
			stroke={cell.color}
			stroke-width={geometry.screenPx(0.04, world)} />
	{/each}

	{#if geometry.selectedDeadzoneSource()}
		{@const deadzoneSource = geometry.selectedDeadzoneSource()}
		{@const deadzoneBounds = geometry.voronoiViewBounds(world)}
		{@const deadzoneHoles = deadzoneSource ? geometry.deadzoneHolesForSource(deadzoneSource) : []}
		<path
			class="deadzone-highlight"
			d={geometry.deadzonePath(deadzoneBounds, deadzoneHoles, world)}
			fill={deadzoneSource?.color ?? 'currentColor'}
			fill-rule="evenodd"
			transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
		{#each deadzoneHoles as hole (hole.key)}
			<circle
				class="deadzone-hole-edge"
				cx={geometry.pointPx(hole.x, world)}
				cy={geometry.pointPx(hole.y, world)}
				r={geometry.pointPx(hole.radius, world)}
				stroke={hole.color}
				stroke-width={geometry.screenPx(0.06, world)}
				stroke-dasharray={geometry.dashArray(world, 0.38, 0.22)}
				transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
		{/each}
	{/if}

	{#if geometry.selectedVoronoiDeadzoneSource()}
		{@const voronoiDeadzoneSource = geometry.selectedVoronoiDeadzoneSource()}
		{@const frozenTargets = voronoiDeadzoneSource
			? geometry.frozenTargetsForSource(voronoiDeadzoneSource)
			: []}
		{#each frozenTargets as target (target.key)}
			{#if target.freezeRadius !== null && target.freezeRadius > 0}
				<circle
					class="deadzone-freeze-highlight"
					cx={geometry.pointPx(target.x, world)}
					cy={geometry.pointPx(target.y, world)}
					r={geometry.pointPx(target.freezeRadius, world)}
					fill={target.color}
					stroke={target.color}
					stroke-width={geometry.screenPx(0.08, world)}
					stroke-dasharray={geometry.dashArray(world, 0.34, 0.2)}
					transition:fade={{ duration: HIGHLIGHT_FADE_MS }} />
			{/if}
		{/each}
	{/if}

	{#if debugView && selectedEndpoint}
		<g class="debug-layer" aria-hidden="true">
			{#each geometry.debugVoronoiComputations as computation (computation.key)}
				{#if computation.current && computation.cellPoints.length > 0}
					<polygon
						class="debug-current-cell"
						points={geometry.polygonPoints(computation.cellPoints, world)}
						fill={computation.current.color}
						stroke={computation.current.color}
						stroke-width={geometry.screenPx(0.06, world)}
						stroke-dasharray={geometry.dashArray(world, 0.46, 0.28)} />
				{/if}
			{/each}

			{#each geometry.debugVoronoiGuides as guide (guide.key)}
				{@const neighborMid = geometry.pointBetween(
					guide.closestPoint,
					guide.neighborMeasurePoint,
					0.5
				)}
				<line
					class="debug-neighbor-edge"
					x1={geometry.pointPx(guide.edgeStart.x, world)}
					y1={geometry.pointPx(guide.edgeStart.y, world)}
					x2={geometry.pointPx(guide.edgeEnd.x, world)}
					y2={geometry.pointPx(guide.edgeEnd.y, world)}
					stroke={guide.neighbor.color}
					stroke-width={geometry.screenPx(0.08, world)} />
				<line
					class="debug-edge-distance"
					x1={geometry.pointPx(guide.source.x, world)}
					y1={geometry.pointPx(guide.source.y, world)}
					x2={geometry.pointPx(guide.closestPoint.x, world)}
					y2={geometry.pointPx(guide.closestPoint.y, world)}
					stroke={guide.current.color}
					stroke-width={geometry.screenPx(0.05, world)}
					stroke-dasharray={geometry.dashArray(world, 0.22, 0.18)} />
				<line
					class="debug-neighbor-distance"
					x1={geometry.pointPx(guide.closestPoint.x, world)}
					y1={geometry.pointPx(guide.closestPoint.y, world)}
					x2={geometry.pointPx(guide.neighborMeasurePoint.x, world)}
					y2={geometry.pointPx(guide.neighborMeasurePoint.y, world)}
					stroke={guide.neighbor.color}
					stroke-width={geometry.screenPx(0.045, world)}
					stroke-dasharray={geometry.dashArray(world, 0.12, 0.2)} />
				<circle
					class="debug-edge-dot"
					cx={geometry.pointPx(guide.closestPoint.x, world)}
					cy={geometry.pointPx(guide.closestPoint.y, world)}
					r={geometry.screenPx(0.12, world)}
					fill={guide.neighbor.color}
					stroke-width={geometry.screenPx(0.05, world)} />
				<text
					class="debug-edge-label"
					x={geometry.pointPx(guide.closestPoint.x, world) + geometry.screenPx(0.18, world)}
					y={geometry.pointPx(guide.closestPoint.y, world) + geometry.screenPx(0.42, world)}
					fill={guide.neighbor.color}
					font-size={geometry.screenPx(0.48, world)}
					stroke-width={geometry.screenPx(0.18, world)}>
					{geometry.debugDistanceLabel(guide.edgeDistance)}
				</text>
				<text
					class="debug-edge-label"
					x={geometry.pointPx(neighborMid.x, world) + geometry.screenPx(0.18, world)}
					y={geometry.pointPx(neighborMid.y, world) - geometry.screenPx(0.18, world)}
					fill={guide.neighbor.color}
					font-size={geometry.screenPx(0.48, world)}
					stroke-width={geometry.screenPx(0.18, world)}>
					{geometry.debugDistanceLabel(guide.neighborDistance)}
				</text>
			{/each}

			{#each geometry.debugConnections as connection (connection.key)}
				{@const mid = geometry.pointBetween(connection.source, connection.target, 0.5)}
				<line
					class="debug-weight-line"
					class:current={connection.role === 'current'}
					class:neighbor={connection.role === 'neighbor'}
					class:frozen={connection.role === 'frozen'}
					x1={geometry.pointPx(connection.source.x, world)}
					y1={geometry.pointPx(connection.source.y, world)}
					x2={geometry.pointPx(connection.target.x, world)}
					y2={geometry.pointPx(connection.target.y, world)}
					stroke={connection.target.color}
					stroke-opacity={geometry.debugLineOpacity(connection.weight)}
					stroke-width={geometry.debugLineWidth(connection.weight, world)}
					stroke-dasharray={geometry.debugConnectionDashArray(connection.role, world)} />
				<circle
					class="debug-weight-dot"
					class:frozen={connection.role === 'frozen'}
					cx={geometry.pointPx(mid.x, world)}
					cy={geometry.pointPx(mid.y, world)}
					r={geometry.screenPx(0.09 + connection.weight * 0.18, world)}
					fill={connection.target.color}
					fill-opacity={geometry.debugLineOpacity(connection.weight)}
					stroke-width={geometry.screenPx(connection.role === 'frozen' ? 0.1 : 0.06, world)} />
				<text
					class="debug-weight-label"
					x={geometry.pointPx(mid.x, world) + geometry.screenPx(0.25, world)}
					y={geometry.pointPx(mid.y, world) - geometry.screenPx(0.18, world)}
					fill={connection.target.color}
					font-size={geometry.screenPx(0.58, world)}
					stroke-width={geometry.screenPx(0.18, world)}>
					{geometry.debugWeightLabel(connection.weight)}
				</text>
			{/each}
		</g>
	{/if}

	{#each targets as endpoint (endpoint.key)}
		<g
			class:disabled={!endpoint.enabled}
			class:selected={isSelected(endpoint)}
			class="endpoint target"
			transform={`translate(${geometry.pointPx(endpoint.x, world)} ${geometry.pointPx(endpoint.y, world)})`}>
			{#if spatializerMode === MODE_VORONOI && endpoint.freezeRadius !== null && endpoint.freezeRadius > 0}
				<circle
					class="freeze-radius-fill"
					r={geometry.pointPx(endpoint.freezeRadius, world)}
					fill={endpoint.color}
					stroke={endpoint.color}
					stroke-width={geometry.screenPx(0.06, world)}
					stroke-dasharray={geometry.dashArray(world, 0.42, 0.26)} />
				{#if endpoint.freezeRadiusWritable}
					<circle
						class="radius-hit freeze-radius-hit"
						role="button"
						tabindex="0"
						aria-label={`Resize ${endpoint.node.meta.label} freeze radius`}
						r={geometry.pointPx(endpoint.freezeRadius, world)}
						stroke-width={geometry.screenPx(0.55, world)}
						onkeydown={(event) => interaction.selectEndpointWithKeyboard(event, endpoint)}
						onpointerdown={(event) =>
							interaction.startEndpointDrag(event, endpoint, 'freezeRadius')} />
				{/if}
			{/if}
			{#if endpoint.radius !== null && endpoint.radius > 0}
				<circle
					class="radius-fill"
					r={geometry.pointPx(endpoint.radius, world)}
					fill={endpoint.color}
					stroke={endpoint.color}
					stroke-width={geometry.screenPx(0.06, world)} />
				{#if endpoint.radiusWritable}
					<circle
						class="radius-hit"
						role="button"
						tabindex="0"
						aria-label={`Resize ${endpoint.node.meta.label} radius`}
						r={geometry.pointPx(endpoint.radius, world)}
						stroke-width={geometry.screenPx(0.55, world)}
						onkeydown={(event) => interaction.selectEndpointWithKeyboard(event, endpoint)}
						onpointerdown={(event) => interaction.startEndpointDrag(event, endpoint, 'radius')} />
				{/if}
			{/if}
			<line
				x1={-geometry.screenPx(0.42, world)}
				y1={-geometry.screenPx(0.42, world)}
				x2={geometry.screenPx(0.42, world)}
				y2={geometry.screenPx(0.42, world)}
				stroke={endpoint.color}
				stroke-width={geometry.screenPx(0.16, world)}
				stroke-linecap="round" />
			<line
				x1={-geometry.screenPx(0.42, world)}
				y1={geometry.screenPx(0.42, world)}
				x2={geometry.screenPx(0.42, world)}
				y2={-geometry.screenPx(0.42, world)}
				stroke={endpoint.color}
				stroke-width={geometry.screenPx(0.16, world)}
				stroke-linecap="round" />
			<circle
				class="endpoint-hit"
				role="button"
				tabindex="0"
				aria-label={`Move ${endpoint.node.meta.label}`}
				r={geometry.screenPx(0.72, world)}
				onkeydown={(event) => interaction.selectEndpointWithKeyboard(event, endpoint)}
				onpointerdown={(event) => interaction.startEndpointDrag(event, endpoint, 'position')} />
		</g>
	{/each}

	{#each sources as endpoint (endpoint.key)}
		<g
			class:disabled={!endpoint.enabled}
			class:selected={isSelected(endpoint)}
			class="endpoint source"
			transform={`translate(${geometry.pointPx(endpoint.x, world)} ${geometry.pointPx(endpoint.y, world)})`}>
			{#if endpoint.radius !== null && endpoint.radius > 0}
				<circle
					class="radius-fill"
					r={geometry.pointPx(endpoint.radius, world)}
					fill={endpoint.color}
					stroke={endpoint.color}
					stroke-width={geometry.screenPx(0.06, world)} />
				{#if endpoint.radiusWritable}
					<circle
						class="radius-hit"
						role="button"
						tabindex="0"
						aria-label={`Resize ${endpoint.node.meta.label} radius`}
						r={geometry.pointPx(endpoint.radius, world)}
						stroke-width={geometry.screenPx(0.55, world)}
						onkeydown={(event) => interaction.selectEndpointWithKeyboard(event, endpoint)}
						onpointerdown={(event) => interaction.startEndpointDrag(event, endpoint, 'radius')} />
				{/if}
			{/if}
			<circle
				class="source-dot"
				r={geometry.screenPx(0.36, world)}
				fill={endpoint.color}
				stroke={endpoint.color}
				stroke-width={geometry.screenPx(0.12, world)} />
			<circle
				class="endpoint-hit"
				role="button"
				tabindex="0"
				aria-label={`Move ${endpoint.node.meta.label}`}
				r={geometry.screenPx(0.72, world)}
				onkeydown={(event) => interaction.selectEndpointWithKeyboard(event, endpoint)}
				onpointerdown={(event) => interaction.startEndpointDrag(event, endpoint, 'position')} />
		</g>
	{/each}
</svg>

<style>
	.spatializer-map {
		position: absolute;
		inset: 0 auto auto 0;
		inline-size: 0.1rem;
		block-size: 0.1rem;
		overflow: visible;
	}

	.voronoi-cell {
		fill-opacity: 0.16;
		stroke-opacity: 0.32;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition),
			filter var(--spatializer-highlight-transition);
	}

	.voronoi-cell.highlighted {
		fill-opacity: 0.2;
		stroke-opacity: 0.44;
	}

	.deadzone-highlight {
		fill-opacity: 0.09;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition);
	}

	.deadzone-hole-edge {
		fill: none;
		stroke-opacity: 0.34;
		pointer-events: none;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.deadzone-freeze-highlight {
		fill-opacity: 0.075;
		stroke-opacity: 0.38;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-layer {
		pointer-events: none;
	}

	.debug-current-cell {
		fill-opacity: 0.05;
		stroke-opacity: 0.42;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-neighbor-edge {
		stroke-opacity: 0.9;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-edge-distance {
		stroke-opacity: 0.68;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-neighbor-distance {
		stroke-opacity: 0.46;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.debug-edge-dot {
		fill-opacity: 0.86;
		stroke: var(--gc-color-background);
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.debug-edge-label,
	.debug-weight-label {
		paint-order: stroke;
		stroke: var(--gc-color-background);
		stroke-linejoin: round;
		font-weight: 750;
		pointer-events: none;
		user-select: none;
	}

	.debug-edge-label {
		opacity: 0.82;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			opacity var(--spatializer-highlight-transition);
	}

	.debug-weight-line {
		stroke-opacity: 0.72;
		stroke-linecap: round;
		transition:
			stroke var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.debug-weight-line.current {
		stroke-opacity: 0.9;
	}

	.debug-weight-line.frozen {
		stroke-opacity: 0.95;
	}

	.debug-weight-dot {
		stroke: color-mix(in srgb, var(--gc-color-background) 80%, transparent);
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-width var(--spatializer-highlight-transition);
	}

	.endpoint {
		cursor: grab;
	}

	.endpoint.disabled {
		opacity: 0.42;
	}

	.endpoint.selected .source-dot,
	.endpoint.selected .freeze-radius-fill,
	.endpoint.selected line {
		filter: drop-shadow(0 0 0.22rem var(--gc-color-accent));
	}

	.radius-fill {
		fill-opacity: 0.08;
		stroke-opacity: 0.48;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.freeze-radius-fill {
		fill-opacity: 0.04;
		stroke-opacity: 0.62;
		pointer-events: none;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition),
			stroke-opacity var(--spatializer-highlight-transition);
	}

	.radius-hit {
		fill: none;
		stroke: transparent;
		cursor: ew-resize;
	}

	.source-dot {
		fill-opacity: 0.95;
		transition:
			fill var(--spatializer-highlight-transition),
			stroke var(--spatializer-highlight-transition),
			fill-opacity var(--spatializer-highlight-transition);
	}

	.endpoint-hit {
		fill: transparent;
		stroke: transparent;
	}
</style>
