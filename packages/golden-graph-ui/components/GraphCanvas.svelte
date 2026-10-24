<script lang="ts">
	import {
		DEFAULT_MIN_ZOOM,
		DEFAULT_MAX_ZOOM,
		type CanvasProps,
		type GraphCanvasProps
	} from './canvas/contracts';
	import { createCanvasViewport, type CanvasViewport } from './canvas/viewport.svelte';
	import { createCanvasGeometry, type CanvasGeometry } from './canvas/geometry.svelte';
	import { createCanvasEditing, type CanvasEditing } from './canvas/editing.svelte';
	import { createCanvasGestures, type CanvasGestures } from './canvas/gestures.svelte';
	import NodeView from './canvas/NodeView.svelte';

	let {
		graphDocument,
		selectedNodeIds = [],
		selectedEdgeIds = [],
		onGraphSelectionChange,
		onNodeMove,
		onNodesMove,
		onNodeResize,
		onNodeRename,
		onNodeCollapsedChange,
		onNodeEnabledChange,
		onConnect,
		canConnect,
		nodeContent,
		nodeHeaderContent,
		inputSocketContent,
		outputSocketContent,
		worldContent,
		worldBounds,
		toolbarEnd,
		onNodeContextMenu,
		onBackgroundContextMenu,
		onBackgroundDoubleClick,
		onCreateRequest,
		routeEdgesAroundNodes = false,
		socketLabels = 'hover',
		initialCamera,
		onCameraChange,
		onVisibleNodeIdsChange,
		viewportInset = {},
		minZoom = DEFAULT_MIN_ZOOM,
		maxZoom = DEFAULT_MAX_ZOOM,
		autoHomeOnMount = true,
		emptyLabel = 'No nodes in this graph.'
	}: GraphCanvasProps = $props();

	const canvasProps: CanvasProps = {
		get graphDocument() {
			return graphDocument;
		},
		get selectedNodeIds() {
			return selectedNodeIds;
		},
		get selectedEdgeIds() {
			return selectedEdgeIds;
		},
		get onGraphSelectionChange() {
			return onGraphSelectionChange;
		},
		get onNodeMove() {
			return onNodeMove;
		},
		get onNodesMove() {
			return onNodesMove;
		},
		get onNodeResize() {
			return onNodeResize;
		},
		get onNodeRename() {
			return onNodeRename;
		},
		get onNodeCollapsedChange() {
			return onNodeCollapsedChange;
		},
		get onNodeEnabledChange() {
			return onNodeEnabledChange;
		},
		get onConnect() {
			return onConnect;
		},
		get canConnect() {
			return canConnect;
		},
		get nodeContent() {
			return nodeContent;
		},
		get nodeHeaderContent() {
			return nodeHeaderContent;
		},
		get inputSocketContent() {
			return inputSocketContent;
		},
		get outputSocketContent() {
			return outputSocketContent;
		},
		get worldContent() {
			return worldContent;
		},
		get worldBounds() {
			return worldBounds;
		},
		get toolbarEnd() {
			return toolbarEnd;
		},
		get onNodeContextMenu() {
			return onNodeContextMenu;
		},
		get onBackgroundContextMenu() {
			return onBackgroundContextMenu;
		},
		get onBackgroundDoubleClick() {
			return onBackgroundDoubleClick;
		},
		get onCreateRequest() {
			return onCreateRequest;
		},
		get routeEdgesAroundNodes() {
			return routeEdgesAroundNodes;
		},
		get socketLabels() {
			return socketLabels;
		},
		get initialCamera() {
			return initialCamera;
		},
		get onCameraChange() {
			return onCameraChange;
		},
		get onVisibleNodeIdsChange() {
			return onVisibleNodeIdsChange;
		},
		get viewportInset() {
			return viewportInset;
		},
		get minZoom() {
			return minZoom;
		},
		get maxZoom() {
			return maxZoom;
		},
		get autoHomeOnMount() {
			return autoHomeOnMount;
		},
		get emptyLabel() {
			return emptyLabel;
		}
	};
	const viewport: CanvasViewport = createCanvasViewport(canvasProps, {
		get effectiveNodes() {
			return geometry.effectiveNodes;
		},
		get selectedIds() {
			return gestures.selectedIds;
		},
		nodeById(id) {
			return geometry.nodeById(id);
		},
		nodeWidth(node) {
			return geometry.nodeWidth(node);
		},
		nodeHeight(node) {
			return geometry.nodeHeight(node);
		}
	});
	const editing: CanvasEditing = createCanvasEditing(canvasProps, {
		sourceNode(id) {
			return geometry.sourceNode(id);
		},
		get selectedIds() {
			return gestures.selectedIds;
		},
		updateSelection(nodeId, additive) {
			gestures.updateSelection(nodeId, additive);
		},
		startNodeDrag(event, node) {
			gestures.startNodeDrag(event, node);
		}
	});
	const geometry: CanvasGeometry = createCanvasGeometry(canvasProps, viewport, {
		get dragPositions() {
			return editing.dragPositions;
		},
		get optimisticPositions() {
			return editing.optimisticPositions;
		},
		get resizeSizes() {
			return editing.resizeSizes;
		},
		get optimisticSizes() {
			return editing.optimisticSizes;
		},
		get optimisticLabels() {
			return editing.optimisticLabels;
		},
		get optimisticCollapsed() {
			return editing.optimisticCollapsed;
		},
		get optimisticEnabled() {
			return editing.optimisticEnabled;
		},
		get connectionDraft() {
			return gestures.connectionDraft;
		}
	});
	const gestures: CanvasGestures = createCanvasGestures(canvasProps, viewport, geometry, editing);

	export const frameSelection = viewport.frameSelection;
	export const home = viewport.home;
	export const focus = viewport.focus;
	export const clientToWorld = viewport.clientToWorld;
	export const viewportCenter = viewport.viewportCenter;
	export const previewNodeMoves = editing.previewMoves;
	export const previewNodeResize = editing.previewResize;
	export const clearNodePreviews = editing.clearPreviews;
	export const presentationWork = geometry.presentationWork;
	export const routingWork = geometry.routingWork;
	let nodes = $derived(geometry.nodes);
	let effectiveMaxZoom = $derived(viewport.effectiveMaxZoom);
	let effectiveMinZoom = $derived(viewport.effectiveMinZoom);
	let camera = $derived(viewport.camera);
	let remPx = $derived(viewport.remPx);
	let viewportWidth = $derived(viewport.viewportWidth);
	let viewportHeight = $derived(viewport.viewportHeight);
	let panGesture = $derived(gestures.panGesture);
	let nodeDragGesture = $derived(gestures.nodeDragGesture);
	let nodeResizeGesture = $derived(gestures.nodeResizeGesture);
	let selectionGesture = $derived(gestures.selectionGesture);
	let connectionDraft = $derived(gestures.connectionDraft);
	let selectedEdgeIdSet = $derived(gestures.selectedEdgeIdSet);
	let nodeCount = $derived(geometry.nodeCount);
	let checkerSizePx = $derived(viewport.checkerSizePx);
	let snappedCameraX = $derived(viewport.snappedCameraX);
	let snappedCameraY = $derived(viewport.snappedCameraY);
	let panStyle = $derived(viewport.panStyle);
	let selectionBoxStyle = $derived(gestures.selectionBoxStyle);
	let visibleNodes = $derived(geometry.visibleNodes);
	let visibleEdges = $derived(geometry.visibleEdges);
	const socketPoint = geometry.socketPoint;
	const edgePath = geometry.edgePath;
	const edgeUsesGradient = geometry.edgeUsesGradient;
	const edgeGradientId = geometry.edgeGradientId;
	const edgeStroke = geometry.edgeStroke;
	const draftPath = geometry.draftPath;
	const resetZoom = viewport.resetZoom;
	const handleZoomInput = viewport.handleZoomInput;
	const selectEdge = gestures.selectEdge;
	const selectEdgeWithKeyboard = gestures.selectEdgeWithKeyboard;
	const startPan = gestures.startPan;
	const handlePointerMove = gestures.handlePointerMove;
	const handlePointerEnd = gestures.handlePointerEnd;
	const resetScrollHoverTarget = viewport.resetScrollHoverTarget;
	const handleWheel = viewport.handleWheel;
	const handleContextMenu = gestures.handleContextMenu;
	const handleDoubleClick = gestures.handleDoubleClick;
	const handleKeydown = gestures.handleKeydown;
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
	bind:this={viewport.container}
	class:panning={panGesture !== null}
	class:node-dragging={nodeDragGesture !== null}
	class:node-resizing={nodeResizeGesture !== null}
	class:selecting={selectionGesture !== null}
	class:connecting={connectionDraft !== null}
	class="graph-canvas"
	data-socket-labels={socketLabels}
	data-local-context-menu={onBackgroundContextMenu ? '' : undefined}
	data-node-count={nodeCount}
	data-visible-node-count={visibleNodes.length}
	role="application"
	aria-label="Node graph"
	tabindex="0"
	style:--checker-size={`${checkerSizePx}px`}
	style:--camera-x={`${snappedCameraX}px`}
	style:--camera-y={`${snappedCameraY}px`}
	style:--camera-zoom={`${camera.zoom}`}
	onpointerdown={startPan}
	onpointermove={handlePointerMove}
	onpointerup={handlePointerEnd}
	onpointercancel={handlePointerEnd}
	onpointerleave={resetScrollHoverTarget}
	onkeydown={handleKeydown}
	onwheel={handleWheel}
	oncontextmenu={handleContextMenu}
	ondblclick={handleDoubleClick}>
	<div
		class="toolbar"
		role="group"
		aria-label="Graph view controls"
		onpointerdown={(event) => event.stopPropagation()}>
		<button type="button" title="Frame selection (F)" onclick={frameSelection}>Frame</button>
		<button type="button" title="Home (H)" onclick={home}>Home</button>
		<div class="zoom-control">
			<input
				class="zoom-slider"
				type="range"
				min={effectiveMinZoom * 100}
				max={effectiveMaxZoom * 100}
				step="1"
				value={camera.zoom * 100}
				aria-label="Zoom"
				title="Zoom"
				oninput={handleZoomInput} />
			<button
				type="button"
				class="zoom-reset"
				aria-label="Reset zoom to 100%"
				title="Reset zoom to 100%"
				onclick={resetZoom}>
				{Math.round(camera.zoom * 100)}%
			</button>
		</div>
		{#if toolbarEnd}
			<span class="toolbar-divider" aria-hidden="true"></span>
			{@render toolbarEnd()}
		{/if}
	</div>

	<div class="world-pan" style:transform={panStyle}>
		<div class="world">
			{#if worldContent}
				<div class="world-content">
					{@render worldContent({
						camera,
						remPx,
						viewportWidth,
						viewportHeight
					})}
				</div>
			{/if}

			<svg class="wires" aria-label="Graph connections">
				{#each visibleEdges as { edge, index } (`${edge.id ?? index}:${edge.from.nodeId}:${edge.to.nodeId}`)}
					{@const path = edgePath(index)}
					{@const start = socketPoint(edge.from, 'output')}
					{@const end = socketPoint(edge.to, 'input')}
					{@const gradientId = edgeGradientId(edge, index)}
					{@const usesGradient = edgeUsesGradient(edge)}
					{#if path}
						{#if start && end && usesGradient}
							<defs>
								<linearGradient
									id={gradientId}
									gradientUnits="userSpaceOnUse"
									x1={start.x}
									y1={start.y}
									x2={end.x}
									y2={end.y}>
									<stop offset="0%" stop-color={edge.color} />
									<stop offset="100%" stop-color={edge.targetColor} />
								</linearGradient>
							</defs>
						{/if}
						<path
							d={path}
							class="edge-hit"
							role="button"
							tabindex="0"
							aria-label="Select connection"
							aria-pressed={edge.id !== undefined && selectedEdgeIdSet.has(edge.id)}
							vector-effect="non-scaling-stroke"
							onpointerdown={(event) => selectEdge(event, edge)}
							onkeydown={(event) => selectEdgeWithKeyboard(event, edge)} />
						<path
							d={path}
							class="edge"
							class:active={edge.active}
							class:gradient={usesGradient}
							class:invalid={edge.invalid}
							class:selected={edge.id !== undefined && selectedEdgeIdSet.has(edge.id)}
							style:--edge-color={edgeStroke(edge, gradientId)}
							vector-effect="non-scaling-stroke" />
					{/if}
				{/each}
				{#if draftPath()}
					<path class="draft" d={draftPath() ?? ''} vector-effect="non-scaling-stroke" />
				{/if}
			</svg>

			{#each visibleNodes as node (node.id)}
				<NodeView {node} {geometry} {editing} {gestures} {canvasProps} />
			{/each}
		</div>
	</div>

	{#if selectionGesture}
		<div class="selection-box" style={selectionBoxStyle}></div>
	{/if}

	{#if nodes.length === 0 && !worldContent}
		<p class="empty">{emptyLabel}</p>
	{/if}
</div>

<style>
	.graph-canvas {
		--ga-bg: var(--gc-color-background, #11151b);
		--ga-grid-odd: var(--gc-color-graph-grid-odd, var(--ga-bg));
		--ga-grid-even: var(
			--gc-color-graph-grid-even,
			color-mix(in srgb, var(--gc-color-text, #d8dfeb) 5%, var(--ga-bg))
		);
		--ga-node: color-mix(in srgb, var(--gc-color-background, #11151b) 80%, #293449);
		--ga-outline: var(--gc-color-panel-outline, #536077);
		--ga-selection: var(--gc-color-selection, #66a6ff);
		--ga-active: #f2c01a;
		--ga-error: #ff6d75;
		--ga-socket: #b4c2d8;
		position: relative;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		outline: none;
		background-color: var(--ga-grid-odd);
		background-image: conic-gradient(
			from 90deg,
			var(--ga-grid-even) 25%,
			var(--ga-grid-odd) 0 50%,
			var(--ga-grid-even) 0 75%,
			var(--ga-grid-odd) 0
		);
		background-position: var(--camera-x, 0) var(--camera-y, 0);
		background-size: var(--checker-size) var(--checker-size);
		user-select: none;
		cursor: default;
	}

	.graph-canvas.panning,
	.graph-canvas.node-dragging,
	.graph-canvas.connecting {
		cursor: grabbing;
	}

	.graph-canvas.node-resizing {
		cursor: nwse-resize;
	}

	.graph-canvas.selecting {
		cursor: crosshair;
	}

	.world-pan,
	.world {
		position: absolute;
		inset: 0 auto auto 0;
		transform-origin: 0 0;
	}

	.world-pan {
		will-change: transform;
	}

	.world {
		zoom: var(--camera-zoom, 1);
	}

	.wires {
		position: absolute;
		inset: 0 auto auto 0;
		inline-size: 0.1rem;
		block-size: 0.1rem;
		overflow: visible;
		pointer-events: none;
		/* z-index: 2; */
	}

	.world-content {
		position: absolute;
		inset: 0 auto auto 0;
		inline-size: 0.1rem;
		block-size: 0.1rem;
		overflow: visible;
	}

	.wires path {
		fill: none;
	}

	.wires path.edge {
		stroke: color-mix(
			in srgb,
			var(--edge-color, var(--ga-socket)) 68%,
			var(--gc-color-background, #11151b) 32%
		);
		stroke-width: 0.13rem;
		opacity: 0.72;
		filter: saturate(0.82);
		pointer-events: none;
		transition:
			opacity 0.48s ease,
			stroke 0.48s ease,
			stroke-width 0.48s ease,
			filter 0.48s ease;
	}

	.wires path.edge.gradient {
		stroke: var(--edge-color, var(--ga-socket));
	}

	.wires path.edge.active {
		stroke: color-mix(in srgb, var(--edge-color, var(--ga-active)) 88%, white 12%);
		stroke-width: 0.15rem;
		opacity: 0.86;
		filter: saturate(1.02);
		transition:
			opacity 0.06s ease-out,
			stroke 0.06s ease-out,
			stroke-width 0.06s ease-out,
			filter 0.06s ease-out;
	}

	.wires path.edge.gradient.active {
		stroke: var(--edge-color, var(--ga-active));
	}

	.wires path.edge.selected {
		stroke: var(--ga-selection);
		stroke-width: 0.25rem;
		opacity: 1;
		filter: drop-shadow(0 0 0.24rem color-mix(in srgb, var(--ga-selection) 72%, transparent));
	}

	.wires path.edge.invalid {
		stroke: var(--ga-error);
		stroke-dasharray: 0.4rem 0.3rem;
	}

	.wires path.edge-hit {
		stroke: transparent;
		stroke-width: 0.85rem;
		pointer-events: stroke;
		cursor: pointer;
	}

	.wires path.draft {
		stroke: var(--ga-selection);
		stroke-dasharray: 0.45rem 0.25rem;
	}

	.selection-box {
		position: absolute;
		z-index: 15;
		box-sizing: border-box;
		border: solid 0.08rem color-mix(in srgb, var(--ga-selection) 82%, transparent);
		background: color-mix(in srgb, var(--ga-selection) 14%, transparent);
		pointer-events: none;
	}

	.toolbar {
		position: absolute;
		inset-block-start: 0.7rem;
		inset-inline-end: 0.7rem;
		z-index: 20;
		display: flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.28rem;
		border: solid 0.06rem color-mix(in srgb, var(--ga-outline) 55%, transparent);
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--ga-bg) 84%, transparent);
		backdrop-filter: blur(0.5rem);
	}

	.toolbar button {
		min-block-size: 1.65rem;
		padding: 0.25rem 0.55rem;
		border: 0;
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--ga-outline) 18%, transparent);
		color: inherit;
		font: inherit;
		font-size: 0.67rem;
	}

	.toolbar button {
		cursor: pointer;
	}

	.toolbar button:hover {
		background: color-mix(in srgb, var(--ga-selection) 28%, transparent);
	}

	.zoom-control {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		padding-inline-start: 0.2rem;
	}

	.zoom-slider {
		appearance: none;
		inline-size: 4.8rem;
		block-size: 0.16rem;
		margin: 0;
		padding: 0;
		border: 0;
		border-radius: 999rem;
		background: color-mix(in srgb, var(--ga-outline) 42%, transparent);
		opacity: 0.62;
		cursor: pointer;
		transition: opacity 120ms ease;
	}

	.zoom-slider:hover,
	.zoom-slider:focus-visible {
		opacity: 1;
	}

	.zoom-slider::-webkit-slider-thumb {
		appearance: none;
		inline-size: 0.62rem;
		block-size: 0.62rem;
		border: solid 0.08rem color-mix(in srgb, var(--ga-bg) 70%, transparent);
		border-radius: 50%;
		background: var(--ga-outline);
	}

	.zoom-slider::-moz-range-thumb {
		inline-size: 0.52rem;
		block-size: 0.52rem;
		border: solid 0.08rem color-mix(in srgb, var(--ga-bg) 70%, transparent);
		border-radius: 50%;
		background: var(--ga-outline);
	}

	.toolbar .zoom-reset {
		min-inline-size: 3.15rem;
		padding-inline: 0.4rem;
		font-variant-numeric: tabular-nums;
	}

	.toolbar-divider {
		display: inline-block;
		inline-size: 0.06rem;
		block-size: 1rem;
		background: color-mix(in srgb, var(--ga-outline) 55%, transparent);
		margin-inline: 0.1rem;
		align-self: center;
	}

	.empty {
		position: absolute;
		inset: 50% auto auto 50%;
		margin: 0;
		transform: translate(-50%, -50%);
		opacity: 0.56;
		pointer-events: none;
	}
</style>
