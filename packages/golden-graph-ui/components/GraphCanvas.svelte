<script lang="ts">
	import { onMount, type Snippet } from 'svelte';
	import { SpatialIndex } from '../spatial-index';
	import type {
		GraphCamera,
		GraphConnectionRequest,
		GraphEdge,
		GraphNode,
		GraphNodeBypassConnection,
		GraphNodeCreationRequest,
		GraphNodeMove,
		GraphNodePosition,
		GraphPresentationDocument,
		GraphNodeResize,
		GraphNodeSize,
		GraphSocket,
		GraphSocketDirection,
		GraphSocketRef,
		GraphViewportInset,
		GraphWorldBounds,
		GraphWorldContentContext
	} from '../types';

	interface PanGesture {
		pointerId: number;
		startX: number;
		startY: number;
		cameraX: number;
		cameraY: number;
		moved: boolean;
		clearSelectionOnClick: boolean;
	}

	interface NodeDragGesture {
		pointerId: number;
		startX: number;
		startY: number;
		nodePositions: Record<string, GraphNodePosition>;
	}

	interface NodeResizeGesture {
		pointerId: number;
		nodeId: string;
		startX: number;
		startY: number;
		startSize: GraphNodeSize;
	}

	interface SelectionGesture {
		pointerId: number;
		startX: number;
		startY: number;
		currentX: number;
		currentY: number;
		initialSelectedNodeIds: string[];
	}

	interface ConnectionDraft {
		pointerId: number;
		from: GraphSocketRef;
		endX: number;
		endY: number;
		snapNodeId?: string;
		snapSocketId?: string;
	}

	interface RoutingObstacle {
		left: number;
		top: number;
		right: number;
		bottom: number;
	}

	interface RoutingState {
		x: number;
		y: number;
		direction: number;
		cost: number;
		estimate: number;
		key: string;
	}

	const DEFAULT_MIN_ZOOM = 0.2;
	const DEFAULT_MAX_ZOOM = 2.5;

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
		viewportInset = {},
		minZoom = DEFAULT_MIN_ZOOM,
		maxZoom = DEFAULT_MAX_ZOOM,
		autoHomeOnMount = true,
		emptyLabel = 'No nodes in this graph.'
	}: {
		graphDocument: GraphPresentationDocument;
		selectedNodeIds?: string[];
		selectedEdgeIds?: string[];
		onGraphSelectionChange?: (nodeIds: string[], edgeIds: string[]) => void;
		onNodeMove?: (nodeId: string, position: GraphNodePosition) => void | Promise<void>;
		onNodesMove?: (moves: GraphNodeMove[]) => void | Promise<void>;
		onNodeResize?: (resize: GraphNodeResize) => void | Promise<void>;
		onNodeRename?: (nodeId: string, label: string) => void | Promise<void>;
		onNodeCollapsedChange?: (nodeId: string, collapsed: boolean) => void | Promise<void>;
		onNodeEnabledChange?: (nodeId: string, enabled: boolean) => void | Promise<void>;
		onConnect?: (connection: GraphConnectionRequest) => void;
		canConnect?: (connection: GraphConnectionRequest) => boolean;
		nodeContent?: Snippet<[GraphNode]>;
		nodeHeaderContent?: Snippet<[GraphNode]>;
		inputSocketContent?: Snippet<[GraphNode, GraphSocket]>;
		outputSocketContent?: Snippet<[GraphNode, GraphSocket]>;
		worldContent?: Snippet<[GraphWorldContentContext]>;
		worldBounds?: GraphWorldBounds;
		toolbarEnd?: Snippet;
		onNodeContextMenu?: (event: MouseEvent, nodeId: string) => void;
		onBackgroundContextMenu?: (event: MouseEvent, position: GraphNodePosition) => void;
		onBackgroundDoubleClick?: (event: MouseEvent, position: GraphNodePosition) => void;
		onCreateRequest?: (request: GraphNodeCreationRequest) => void;
		routeEdgesAroundNodes?: boolean;
		socketLabels?: 'always' | 'hover' | 'never';
		initialCamera?: GraphCamera;
		onCameraChange?: (camera: GraphCamera) => void;
		viewportInset?: GraphViewportInset;
		minZoom?: number;
		maxZoom?: number;
		autoHomeOnMount?: boolean;
		emptyLabel?: string;
	} = $props();
	let nodes = $derived(graphDocument.nodes);
	let edges = $derived(graphDocument.edges);
	let topologyRevision = $derived(graphDocument.revision.topology);
	let presentationRevision = $derived(graphDocument.revision.presentation);

	const CHECKER_CELL_REM = 2;
	const DEFAULT_NODE_WIDTH_REM = 13;
	const MIN_NODE_WIDTH_REM = 8;
	const NODE_BORDER_REM = 0.08;
	const NODE_HEADER_REM = 1.8;
	const SOCKET_ROW_REM = 1.35;
	const HEADER_SOCKET_INSET_REM = 0.47;
	const FRAME_PADDING_REM = 3;
	const CAMERA_ANIMATION_MS = 240;
	const ROUTING_GRID_REM = 1;
	const ROUTING_CLEARANCE_REM = 0.7;
	const ROUTING_MARGIN_REM = 16;
	const ROUTING_BUCKET_REM = 16;
	const ROUTING_MAX_VISITS = 24_000;
	const SCROLL_TARGET_ACTIVATION_MS = 300;
	const ROUTING_TURN_COST = 0.35;
	const TITLE_DOUBLE_CLICK_MS = 300;
	const TITLE_DOUBLE_CLICK_DISTANCE_PX = 6;

	const finiteNumber = (value: number | undefined, fallback: number): number =>
		typeof value === 'number' && Number.isFinite(value) ? value : fallback;

	let effectiveMaxZoom = $derived(Math.max(0.01, finiteNumber(maxZoom, DEFAULT_MAX_ZOOM)));
	let effectiveMinZoom = $derived(
		Math.min(effectiveMaxZoom, Math.max(0.01, finiteNumber(minZoom, DEFAULT_MIN_ZOOM)))
	);

	const normalizeCamera = (value: GraphCamera | undefined): GraphCamera => ({
		x: finiteNumber(value?.x, 0),
		y: finiteNumber(value?.y, 0),
		zoom: Math.min(effectiveMaxZoom, Math.max(effectiveMinZoom, finiteNumber(value?.zoom, 1)))
	});

	const cameraKey = (value: GraphCamera): string => `${value.x}:${value.y}:${value.zoom}`;

	const snapScreenPx = (value: number): number => {
		const scale = typeof window === 'undefined' ? 1 : Math.max(1, window.devicePixelRatio || 1);
		return Math.round(value * scale) / scale;
	};

	let container: HTMLDivElement | null = $state(null);
	let camera = $state<GraphCamera>({ x: 0, y: 0, zoom: 1 });
	let appliedInitialCameraKey = $state<string | null>(null);
	let remPx = $state(16);
	let viewportWidth = $state(1);
	let viewportHeight = $state(1);
	let panGesture = $state<PanGesture | null>(null);
	let nodeDragGesture = $state<NodeDragGesture | null>(null);
	let nodeResizeGesture = $state<NodeResizeGesture | null>(null);
	let selectionGesture = $state<SelectionGesture | null>(null);
	let connectionDraft = $state<ConnectionDraft | null>(null);
	let hoveredScrollTarget: HTMLElement | null = null;
	let wheelScrollTarget: HTMLElement | null = null;
	let scrollActivationTimer: ReturnType<typeof setTimeout> | null = null;
	let dragPositions = $state<Record<string, GraphNodePosition>>({});
	let optimisticPositions = $state<Record<string, GraphNodePosition>>({});
	let resizeSizes = $state<Record<string, GraphNodeSize>>({});
	let optimisticSizes = $state<Record<string, GraphNodeSize | null>>({});
	let optimisticLabels = $state<Record<string, string>>({});
	let optimisticCollapsed = $state<Record<string, boolean>>({});
	let optimisticEnabled = $state<Record<string, boolean>>({});
	let expandedSockets = $state<Record<string, boolean>>({});
	let renamingNodeId: string | null = $state(null);
	let renameDraft = $state('');
	let renameInput: HTMLInputElement | null = $state(null);
	let focusedRenameNodeId: string | null = null;
	let lastTitlePointerDown: { nodeId: string; time: number; x: number; y: number } | null = null;
	let lastResizePointerDown: { nodeId: string; time: number; x: number; y: number } | null = null;
	let animationFrame: number | null = null;
	const routedPathCache = new Map<string, string>();

	let selectedIds = $derived(new Set(selectedNodeIds));
	let selectedEdgeIdSet = $derived(new Set(selectedEdgeIds));
	let effectiveNodes = $derived(
		nodes.map((node) => {
			const position = dragPositions[node.id] ?? optimisticPositions[node.id];
			const size = node.id in resizeSizes ? resizeSizes[node.id] : optimisticSizes[node.id];
			const label = optimisticLabels[node.id];
			const collapsed = optimisticCollapsed[node.id];
			const enabled = optimisticEnabled[node.id];
			return position !== undefined ||
				size !== undefined ||
				label !== undefined ||
				collapsed !== undefined ||
				enabled !== undefined
				? {
						...node,
						...(position !== undefined ? { position } : {}),
						...(size !== undefined ? { size: size ?? undefined } : {}),
						...(label !== undefined ? { label } : {}),
						...(collapsed !== undefined ? { collapsed } : {}),
						...(enabled !== undefined ? { enabled } : {})
					}
				: node;
		})
	);
	let nodesById = $derived(new Map(effectiveNodes.map((node) => [node.id, node])));
	let checkerCellSizePx = $derived(Math.max(8, remPx * CHECKER_CELL_REM * camera.zoom));
	let checkerSizePx = $derived(checkerCellSizePx * 2);
	let snappedCameraX = $derived(snapScreenPx(camera.x));
	let snappedCameraY = $derived(snapScreenPx(camera.y));
	let panStyle = $derived(`translate(${snappedCameraX}px, ${snappedCameraY}px)`);
	let selectionBoxStyle = $derived.by(() => {
		if (!selectionGesture) {
			return '';
		}
		const left = Math.min(selectionGesture.startX, selectionGesture.currentX);
		const top = Math.min(selectionGesture.startY, selectionGesture.currentY);
		const width = Math.abs(selectionGesture.currentX - selectionGesture.startX);
		const height = Math.abs(selectionGesture.currentY - selectionGesture.startY);
		return `left:${left}px;top:${top}px;width:${width}px;height:${height}px`;
	});

	$effect(() => {
		let next = optimisticPositions;
		for (const node of nodes) {
			const optimistic = next[node.id];
			if (
				optimistic &&
				Math.abs(node.position.x - optimistic.x) < 0.0001 &&
				Math.abs(node.position.y - optimistic.y) < 0.0001
			) {
				if (next === optimisticPositions) {
					next = { ...optimisticPositions };
				}
				delete next[node.id];
			}
		}
		if (next !== optimisticPositions) {
			optimisticPositions = next;
		}
	});

	$effect(() => {
		let next = optimisticSizes;
		for (const node of nodes) {
			const optimistic = next[node.id];
			if (
				optimistic !== undefined &&
				((optimistic === null && node.size === undefined) ||
					(optimistic !== null &&
						node.size !== undefined &&
						Math.abs(node.size.width - optimistic.width) < 0.0001 &&
						Math.abs(node.size.height - optimistic.height) < 0.0001))
			) {
				if (next === optimisticSizes) {
					next = { ...optimisticSizes };
				}
				delete next[node.id];
			}
		}
		if (next !== optimisticSizes) {
			optimisticSizes = next;
		}
	});

	$effect(() => {
		let next = optimisticLabels;
		const labelsById = new Map(nodes.map((node) => [node.id, node.label]));
		for (const [nodeId, label] of Object.entries(optimisticLabels)) {
			if (labelsById.get(nodeId) === label || !labelsById.has(nodeId)) {
				if (next === optimisticLabels) {
					next = { ...optimisticLabels };
				}
				delete next[nodeId];
			}
		}
		if (next !== optimisticLabels) {
			optimisticLabels = next;
		}
	});

	$effect(() => {
		let next = optimisticCollapsed;
		const collapsedById = new Map(nodes.map((node) => [node.id, node.collapsed === true]));
		for (const [nodeId, collapsed] of Object.entries(optimisticCollapsed)) {
			if (collapsedById.get(nodeId) === collapsed || !collapsedById.has(nodeId)) {
				if (next === optimisticCollapsed) {
					next = { ...optimisticCollapsed };
				}
				delete next[nodeId];
			}
		}
		if (next !== optimisticCollapsed) {
			optimisticCollapsed = next;
		}
	});

	$effect(() => {
		let next = optimisticEnabled;
		const enabledById = new Map(nodes.map((node) => [node.id, node.enabled !== false]));
		for (const [nodeId, enabled] of Object.entries(optimisticEnabled)) {
			if (enabledById.get(nodeId) === enabled || !enabledById.has(nodeId)) {
				if (next === optimisticEnabled) {
					next = { ...optimisticEnabled };
				}
				delete next[nodeId];
			}
		}
		if (next !== optimisticEnabled) {
			optimisticEnabled = next;
		}
	});

	$effect(() => {
		if (!renameInput || !renamingNodeId || focusedRenameNodeId === renamingNodeId) {
			return;
		}
		focusedRenameNodeId = renamingNodeId;
		requestAnimationFrame(() => {
			renameInput?.focus();
			renameInput?.select();
		});
	});

	const clamp = (value: number, minimum: number, maximum: number): number =>
		Math.min(maximum, Math.max(minimum, value));

	const normalizedViewportInset = (): Required<GraphViewportInset> => {
		const left = Math.max(0, finiteNumber(viewportInset.left, 0));
		const right = Math.max(0, finiteNumber(viewportInset.right, 0));
		const top = Math.max(0, finiteNumber(viewportInset.top, 0));
		const bottom = Math.max(0, finiteNumber(viewportInset.bottom, 0));
		const horizontalScale = Math.min(1, viewportWidth / Math.max(1, left + right));
		const verticalScale = Math.min(1, viewportHeight / Math.max(1, top + bottom));
		return {
			left: left * horizontalScale,
			right: right * horizontalScale,
			top: top * verticalScale,
			bottom: bottom * verticalScale
		};
	};

	const visibleViewport = (): { x: number; y: number; width: number; height: number } => {
		const inset = normalizedViewportInset();
		const width = Math.max(1, viewportWidth - inset.left - inset.right);
		const height = Math.max(1, viewportHeight - inset.top - inset.bottom);
		return {
			x: inset.left,
			y: inset.top,
			width,
			height
		};
	};

	const visibleViewportCenter = (): GraphNodePosition => {
		const viewport = visibleViewport();
		return {
			x: viewport.x + viewport.width * 0.5,
			y: viewport.y + viewport.height * 0.5
		};
	};

	const camerasMatch = (left: GraphCamera, right: GraphCamera): boolean =>
		Math.abs(left.x - right.x) < 0.0001 &&
		Math.abs(left.y - right.y) < 0.0001 &&
		Math.abs(left.zoom - right.zoom) < 0.0001;

	const applyCamera = (next: GraphCamera, notify = true): void => {
		const normalized = normalizeCamera(next);
		camera = normalized;
		if (notify) {
			onCameraChange?.({ ...normalized });
		}
	};

	const nodeAutomaticSize = (node: GraphNode): GraphNodeSize | undefined => {
		const size = node.automaticSize;
		return size && size.width > 0 && size.height > 0 ? size : undefined;
	};

	const nodeWidth = (node: GraphNode): number =>
		Math.max(
			MIN_NODE_WIDTH_REM,
			node.size?.width ?? nodeAutomaticSize(node)?.width ?? DEFAULT_NODE_WIDTH_REM
		);

	const headerSublineText = (node: GraphNode): string =>
		node.description?.trim() || node.subtitle?.trim() || '';

	const wrappedLineCount = (text: string, node: GraphNode, averageCharWidthRem: number): number => {
		const trimmed = text.trim();
		if (!trimmed) {
			return 0;
		}
		const socketSpace =
			(node.collapsed === true
				? (allNodeSockets(node, 'input').length > 0 ? 1.8 : 0) +
					(allNodeSockets(node, 'output').length > 0 ? 1.8 : 0)
				: (node.socketPlacement === 'header' ? node.inputs.length + node.outputs.length : 0) *
					1.55) +
			((node.headerInputs?.length ?? 0) > 0 && node.socketPlacement !== 'header' ? 1.2 : 0);
		const availableRem = Math.max(4.5, nodeWidth(node) - socketSpace - 0.9);
		const charsPerLine = Math.max(8, Math.floor(availableRem / averageCharWidthRem));
		return Math.max(1, Math.ceil(trimmed.length / charsPerLine));
	};

	const nodeHeaderHeight = (node: GraphNode): number => {
		const titleLines = wrappedLineCount(node.label, node, 0.43);
		const sublineLines = wrappedLineCount(headerSublineText(node), node, 0.34);
		return Math.max(
			NODE_HEADER_REM,
			0.34 + Math.max(1, titleLines) * 0.82 + sublineLines * 0.66 + 0.32
		);
	};

	const socketStart = (node: GraphNode): number => nodeHeaderHeight(node) + 0.25 + NODE_BORDER_REM;

	const rootNodeSockets = (node: GraphNode, direction: GraphSocketDirection): GraphSocket[] =>
		direction === 'input' ? [...node.inputs, ...(node.headerInputs ?? [])] : node.outputs;

	const flattenSockets = (sockets: readonly GraphSocket[]): GraphSocket[] =>
		sockets.flatMap((socket) => [socket, ...flattenSockets(socket.children ?? [])]);

	const allNodeSockets = (node: GraphNode, direction: GraphSocketDirection): GraphSocket[] =>
		flattenSockets(rootNodeSockets(node, direction));

	const socketRefKey = (nodeId: string, socketId: string): string => `${nodeId}:${socketId}`;

	const socketExpansionKey = (
		node: GraphNode,
		socket: GraphSocket,
		direction: GraphSocketDirection
	): string => `${node.id}:${direction}:${socket.id}`;

	const hasSocketConnection = (node: GraphNode, socketId: string): boolean =>
		connectedSockets.has(socketRefKey(node.id, socketId));

	const socketHasConnectedChild = (node: GraphNode, socket: GraphSocket): boolean =>
		(socket.children ?? []).some((child) => hasSocketConnection(node, child.id));

	const socketExpanded = (
		node: GraphNode,
		socket: GraphSocket,
		direction: GraphSocketDirection
	): boolean =>
		(socket.children?.length ?? 0) > 0 &&
		(socketHasConnectedChild(node, socket) ||
			expandedSockets[socketExpansionKey(node, socket, direction)] === true);

	const displaySockets = (
		node: GraphNode,
		direction: GraphSocketDirection,
		sockets: readonly GraphSocket[]
	): GraphSocket[] =>
		sockets.flatMap((socket) =>
			socketExpanded(node, socket, direction) ? [socket, ...(socket.children ?? [])] : [socket]
		);

	const displayNodeSockets = (node: GraphNode, direction: GraphSocketDirection): GraphSocket[] =>
		direction === 'input'
			? [...displaySockets(node, 'input', node.inputs), ...(node.headerInputs ?? [])]
			: displaySockets(node, 'output', node.outputs);

	const connectionAllowed = (connection: GraphConnectionRequest): boolean =>
		canConnect?.(connection) ?? true;

	const socketDisabled = (
		node: GraphNode,
		socket: GraphSocket,
		direction: GraphSocketDirection
	): boolean => {
		if (socket.compatible === false) return true;
		if (direction === 'output') return false;
		if (socket.parentId && hasSocketConnection(node, socket.parentId)) return true;
		if (!socket.parentId && socketHasConnectedChild(node, socket)) return true;
		if (connectionDraft) {
			return !connectionAllowed({
				from: connectionDraft.from,
				to: { nodeId: node.id, socketId: socket.id }
			});
		}
		return false;
	};

	const inputSocketContentVisible = (node: GraphNode, socket: GraphSocket): boolean =>
		Boolean(inputSocketContent) &&
		Boolean(socket.defaultParamId) &&
		!socket.parentId &&
		!hasSocketConnection(node, socket.id) &&
		!socketHasConnectedChild(node, socket);

	const socketDefaultSlotWidth = (socket: GraphSocket): number => {
		if (!socket.defaultParamId) return 0;
		switch ((socket.valueType ?? '').trim()) {
			case 'vec3':
			case 'color':
				return 8.2;
			case 'vec2':
				return 6.6;
			case 'bool':
				return 1.4;
			case 'int':
			case 'float':
			case 'duration':
				return 8.8;
			default:
				return 5.4;
		}
	};

	const socketDefaultColumnWidth = (sockets: readonly GraphSocket[]): number =>
		sockets.reduce((width, socket) => Math.max(width, socketDefaultSlotWidth(socket)), 0);

	const toggleSocketExpanded = (
		event: MouseEvent,
		node: GraphNode,
		socket: GraphSocket,
		direction: GraphSocketDirection
	): void => {
		event.preventDefault();
		event.stopPropagation();
		const key = socketExpansionKey(node, socket, direction);
		expandedSockets = { ...expandedSockets, [key]: !socketExpanded(node, socket, direction) };
	};

	const firstCollapsedSocket = (
		node: GraphNode,
		direction: GraphSocketDirection
	): GraphSocket | null => {
		const sockets = rootNodeSockets(node, direction);
		return sockets.find((socket) => socket.compatible !== false) ?? sockets[0] ?? null;
	};

	const collapsedSocketLabel = (node: GraphNode, direction: GraphSocketDirection): string => {
		const sockets = rootNodeSockets(node, direction);
		if (sockets.length === 1) {
			return sockets[0].label;
		}
		return direction === 'input' ? 'In' : 'Out';
	};

	const collapsedSocketTitle = (node: GraphNode, direction: GraphSocketDirection): string => {
		const sockets = rootNodeSockets(node, direction);
		if (sockets.length === 1) {
			return sockets[0].valueType ?? sockets[0].label;
		}
		return `${sockets.length} ${direction === 'input' ? 'inputs' : 'outputs'}`;
	};

	const collapsedSocketConnected = (node: GraphNode, direction: GraphSocketDirection): boolean =>
		allNodeSockets(node, direction).some((socket) =>
			connectedSockets.has(`${node.id}:${socket.id}`)
		);

	const collapsedSocketDraftTarget = (node: GraphNode, direction: GraphSocketDirection): boolean =>
		direction === 'input' &&
		connectionDraft?.snapNodeId === node.id &&
		allNodeSockets(node, direction).some((socket) => connectionDraft?.snapSocketId === socket.id);

	const minimumNodeHeight = (node: GraphNode): number =>
		node.collapsed === true
			? nodeHeaderHeight(node) + NODE_BORDER_REM * 2
			: node.socketPlacement === 'header'
				? nodeHeaderHeight(node) + NODE_BORDER_REM * 2 + 1.2
				: Math.max(
						nodeHeaderHeight(node) + NODE_BORDER_REM * 2 + 1.2,
						nodeHeaderHeight(node) +
							NODE_BORDER_REM * 2 +
							0.8 +
							Math.max(
								displayNodeSockets(node, 'input').filter(
									(socket) => !node.headerInputs?.includes(socket)
								).length,
								displayNodeSockets(node, 'output').length,
								1
							) *
								SOCKET_ROW_REM
					);

	const nodeHeight = (node: GraphNode): number =>
		node.collapsed === true
			? minimumNodeHeight(node)
			: Math.max(
					minimumNodeHeight(node),
					node.size?.height ?? nodeAutomaticSize(node)?.height ?? minimumNodeHeight(node)
				);

	let routingGeometryKey = $derived(
		`${remPx}:${effectiveNodes
			.map(
				(node) =>
					`${node.id}:${node.position.x}:${node.position.y}:${nodeWidth(node)}:${nodeHeight(node)}:${nodeHeaderHeight(node)}:${node.collapsed === true}:${node.socketPlacement ?? 'body'}`
			)
			.join('|')}`
	);

	$effect(() => {
		routingGeometryKey;
		routedPathCache.clear();
	});

	let nodeSpatialIndex = $derived.by(() => {
		presentationRevision;
		const index = new SpatialIndex<GraphNode>(32);
		for (const node of effectiveNodes) {
			index.insert(
				node.id,
				{
					left: node.position.x,
					top: node.position.y,
					right: node.position.x + nodeWidth(node),
					bottom: node.position.y + nodeHeight(node)
				},
				node
			);
		}
		return index;
	});
	let visibleNodes = $derived.by(() => {
		const margin = 8;
		const left = -camera.x / camera.zoom / remPx - margin;
		const top = -camera.y / camera.zoom / remPx - margin;
		const right = (viewportWidth - camera.x) / camera.zoom / remPx + margin;
		const bottom = (viewportHeight - camera.y) / camera.zoom / remPx + margin;
		return nodeSpatialIndex.query({ left, top, right, bottom });
	});
	let visibleNodeIds = $derived(new Set(visibleNodes.map((node) => node.id)));
	let edgeIndexesByNodeId = $derived.by(() => {
		topologyRevision;
		const indexes = new Map<string, number[]>();
		for (let index = 0; index < edges.length; index += 1) {
			const edge = edges[index];
			for (const nodeId of new Set([edge.from.nodeId, edge.to.nodeId])) {
				const nodeIndexes = indexes.get(nodeId);
				if (nodeIndexes) {
					nodeIndexes.push(index);
				} else {
					indexes.set(nodeId, [index]);
				}
			}
		}
		return indexes;
	});
	let visibleEdges = $derived.by(() => {
		const indexes = new Set<number>();
		for (const nodeId of visibleNodeIds) {
			for (const index of edgeIndexesByNodeId.get(nodeId) ?? []) {
				indexes.add(index);
			}
		}
		return [...indexes]
			.sort((left, right) => left - right)
			.map((index) => edges[index]);
	});
	let connectedSockets = $derived(
		new Set(
			edges.flatMap((edge) => [
				`${edge.from.nodeId}:${edge.from.socketId}`,
				`${edge.to.nodeId}:${edge.to.socketId}`
			])
		)
	);
	let routingObstacles = $derived.by((): RoutingObstacle[] => {
		const clearance = ROUTING_CLEARANCE_REM * remPx;
		return effectiveNodes.map((node) => ({
			left: node.position.x * remPx - clearance,
			top: node.position.y * remPx - clearance,
			right: (node.position.x + nodeWidth(node)) * remPx + clearance,
			bottom: (node.position.y + nodeHeight(node)) * remPx + clearance
		}));
	});
	let routingObstacleBuckets = $derived.by(() => {
		const buckets = new Map<string, RoutingObstacle[]>();
		const bucketSize = ROUTING_BUCKET_REM * remPx;
		for (const obstacle of routingObstacles) {
			const firstX = Math.floor(obstacle.left / bucketSize);
			const lastX = Math.floor(obstacle.right / bucketSize);
			const firstY = Math.floor(obstacle.top / bucketSize);
			const lastY = Math.floor(obstacle.bottom / bucketSize);
			for (let bucketX = firstX; bucketX <= lastX; bucketX += 1) {
				for (let bucketY = firstY; bucketY <= lastY; bucketY += 1) {
					const key = `${bucketX}:${bucketY}`;
					const entries = buckets.get(key);
					if (entries) {
						entries.push(obstacle);
					} else {
						buckets.set(key, [obstacle]);
					}
				}
			}
		}
		return buckets;
	});

	const socketIndex = (
		node: GraphNode,
		socketId: string,
		direction: GraphSocketDirection
	): number => {
		const sockets = displayNodeSockets(node, direction);
		return Math.max(
			0,
			sockets.findIndex((socket) => socket.id === socketId)
		);
	};

	const socketPoint = (
		reference: GraphSocketRef,
		direction: GraphSocketDirection
	): { x: number; y: number } | null => {
		const node = nodesById.get(reference.nodeId);
		if (!node) {
			return null;
		}
		if (
			node.collapsed === true &&
			allNodeSockets(node, direction).some((socket) => socket.id === reference.socketId)
		) {
			return {
				x:
					(node.position.x +
						(direction === 'output'
							? nodeWidth(node) - HEADER_SOCKET_INSET_REM
							: HEADER_SOCKET_INSET_REM)) *
					remPx,
				y: (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) * remPx
			};
		}
		if (node.socketPlacement === 'header') {
			return {
				x:
					(node.position.x +
						(direction === 'output'
							? nodeWidth(node) - HEADER_SOCKET_INSET_REM
							: HEADER_SOCKET_INSET_REM)) *
					remPx,
				y: (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) * remPx
			};
		}
		if (direction === 'input' && node.headerInputs) {
			const headerIdx = node.headerInputs.findIndex((s) => s.id === reference.socketId);
			if (headerIdx >= 0) {
				return {
					x: (node.position.x + HEADER_SOCKET_INSET_REM + headerIdx * SOCKET_ROW_REM * 0.8) * remPx,
					y: (node.position.y + NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5) * remPx
				};
			}
		}
		return {
			x: (node.position.x + (direction === 'output' ? nodeWidth(node) : 0)) * remPx,
			y:
				(node.position.y +
					socketStart(node) +
					(socketIndex(node, reference.socketId, direction) + 0.5) * SOCKET_ROW_REM) *
				remPx
		};
	};

	const localSocketPoint = (
		node: GraphNode,
		socketId: string,
		direction: GraphSocketDirection
	): { x: number; y: number } | null => {
		if (
			node.collapsed === true &&
			allNodeSockets(node, direction).some((socket) => socket.id === socketId)
		) {
			return {
				x:
					direction === 'output'
						? nodeWidth(node) - HEADER_SOCKET_INSET_REM
						: HEADER_SOCKET_INSET_REM,
				y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5
			};
		}
		if (node.socketPlacement === 'header') {
			return {
				x:
					direction === 'output'
						? nodeWidth(node) - HEADER_SOCKET_INSET_REM
						: HEADER_SOCKET_INSET_REM,
				y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5
			};
		}
		if (direction === 'input' && node.headerInputs) {
			const headerIdx = node.headerInputs.findIndex((socket) => socket.id === socketId);
			if (headerIdx >= 0) {
				return {
					x: HEADER_SOCKET_INSET_REM + headerIdx * SOCKET_ROW_REM * 0.8,
					y: NODE_BORDER_REM + nodeHeaderHeight(node) * 0.5
				};
			}
		}
		return {
			x: direction === 'output' ? nodeWidth(node) : 0,
			y: socketStart(node) + (socketIndex(node, socketId, direction) + 0.5) * SOCKET_ROW_REM
		};
	};

	const wirePath = (start: { x: number; y: number }, end: { x: number; y: number }): string => {
		const horizontal = Math.abs(end.x - start.x);
		const control = clamp(horizontal * 0.5, remPx * 2.5, remPx * 12);
		return `M ${start.x} ${start.y} C ${start.x + control} ${start.y}, ${end.x - control} ${end.y}, ${end.x} ${end.y}`;
	};

	const localWirePath = (
		start: { x: number; y: number },
		end: { x: number; y: number }
	): string => {
		const horizontal = Math.abs(end.x - start.x);
		const control = clamp(horizontal * 0.5, 1.2, 6);
		return `M ${start.x} ${start.y} C ${start.x + control} ${start.y}, ${end.x - control} ${end.y}, ${end.x} ${end.y}`;
	};

	const nodeBypassPath = (node: GraphNode, bypass: GraphNodeBypassConnection): string | null => {
		const start = localSocketPoint(node, bypass.inputSocketId, 'input');
		const end = localSocketPoint(node, bypass.outputSocketId, 'output');
		return start && end ? localWirePath(start, end) : null;
	};

	const routingPointBlocked = (x: number, y: number): boolean => {
		const bucketSize = ROUTING_BUCKET_REM * remPx;
		const obstacles =
			routingObstacleBuckets.get(`${Math.floor(x / bucketSize)}:${Math.floor(y / bucketSize)}`) ??
			[];
		return obstacles.some(
			(obstacle) =>
				x >= obstacle.left && x <= obstacle.right && y >= obstacle.top && y <= obstacle.bottom
		);
	};

	const routingStateKey = (x: number, y: number, direction: number): string =>
		`${x}:${y}:${direction}`;

	const heapPush = (heap: RoutingState[], state: RoutingState): void => {
		heap.push(state);
		let index = heap.length - 1;
		while (index > 0) {
			const parent = Math.floor((index - 1) * 0.5);
			if (heap[parent].estimate <= state.estimate) {
				break;
			}
			heap[index] = heap[parent];
			index = parent;
		}
		heap[index] = state;
	};

	const heapPop = (heap: RoutingState[]): RoutingState | null => {
		const first = heap[0];
		const last = heap.pop();
		if (!first || !last || heap.length === 0) {
			return first ?? null;
		}
		let index = 0;
		while (true) {
			const left = index * 2 + 1;
			const right = left + 1;
			if (left >= heap.length) {
				break;
			}
			const child =
				right < heap.length && heap[right].estimate < heap[left].estimate ? right : left;
			if (heap[child].estimate >= last.estimate) {
				break;
			}
			heap[index] = heap[child];
			index = child;
		}
		heap[index] = last;
		return first;
	};

	const simplifyRoute = (points: GraphNodePosition[]): GraphNodePosition[] => {
		const unique = points.filter(
			(point, index) =>
				index === 0 || point.x !== points[index - 1].x || point.y !== points[index - 1].y
		);
		if (unique.length < 3) {
			return unique;
		}
		const simplified = [unique[0]];
		for (let index = 1; index < unique.length - 1; index += 1) {
			const previous = simplified[simplified.length - 1];
			const current = unique[index];
			const next = unique[index + 1];
			if (
				(previous.x === current.x && current.x === next.x) ||
				(previous.y === current.y && current.y === next.y)
			) {
				continue;
			}
			simplified.push(current);
		}
		simplified.push(unique.at(-1) ?? unique[0]);
		return simplified;
	};

	const pointInRect = (
		x: number,
		y: number,
		left: number,
		top: number,
		right: number,
		bottom: number
	) => x > left && x < right && y > top && y < bottom;

	const lineIntersectsAnyObstacle = (x0: number, y0: number, x1: number, y1: number): boolean => {
		const minX = Math.min(x0, x1);
		const maxX = Math.max(x0, x1);
		const minY = Math.min(y0, y1);
		const maxY = Math.max(y0, y1);

		for (const obstacle of routingObstacles) {
			if (
				maxX <= obstacle.left ||
				minX >= obstacle.right ||
				maxY <= obstacle.top ||
				minY >= obstacle.bottom
			) {
				continue;
			}
			if (
				pointInRect(x0, y0, obstacle.left, obstacle.top, obstacle.right, obstacle.bottom) ||
				pointInRect(x1, y1, obstacle.left, obstacle.top, obstacle.right, obstacle.bottom)
			) {
				return true;
			}

			const dx = x1 - x0;
			const dy = y1 - y0;
			if (dx === 0 || dy === 0) {
				if (
					dy === 0 &&
					y0 > obstacle.top &&
					y0 < obstacle.bottom &&
					minX < obstacle.right &&
					maxX > obstacle.left
				)
					return true;
				if (
					dx === 0 &&
					x0 > obstacle.left &&
					x0 < obstacle.right &&
					minY < obstacle.bottom &&
					maxY > obstacle.top
				)
					return true;
				continue;
			}

			const checkLine = (x3: number, y3: number, x4: number, y4: number) => {
				const den = (x0 - x1) * (y3 - y4) - (y0 - y1) * (x3 - x4);
				if (den === 0) return false;
				const t = ((x0 - x3) * (y3 - y4) - (y0 - y3) * (x3 - x4)) / den;
				const u = -((x0 - x1) * (y0 - y3) - (y0 - y1) * (x0 - x3)) / den;
				return t > 0 && t < 1 && u > 0 && u < 1;
			};

			if (
				checkLine(obstacle.left, obstacle.top, obstacle.right, obstacle.top) ||
				checkLine(obstacle.right, obstacle.top, obstacle.right, obstacle.bottom) ||
				checkLine(obstacle.right, obstacle.bottom, obstacle.left, obstacle.bottom) ||
				checkLine(obstacle.left, obstacle.bottom, obstacle.left, obstacle.top)
			) {
				return true;
			}
		}
		return false;
	};

	const smoothRoute = (points: GraphNodePosition[]): GraphNodePosition[] => {
		if (points.length <= 2) return points;

		const smoothed: GraphNodePosition[] = [points[0]];
		let currentIndex = 0;

		while (currentIndex < points.length - 1) {
			let furthestVisibleIndex = currentIndex + 1;

			for (let i = currentIndex + 2; i < points.length; i++) {
				if (
					!lineIntersectsAnyObstacle(
						points[currentIndex].x,
						points[currentIndex].y,
						points[i].x,
						points[i].y
					)
				) {
					furthestVisibleIndex = i;
				}
			}

			smoothed.push(points[furthestVisibleIndex]);
			currentIndex = furthestVisibleIndex;
		}

		return smoothed;
	};

	const roundedPath = (points: GraphNodePosition[], radius: number): string => {
		if (points.length < 3) {
			return points.map((p, i) => `${i === 0 ? 'M' : 'L'} ${p.x} ${p.y}`).join(' ');
		}
		let path = `M ${points[0].x} ${points[0].y}`;
		for (let i = 1; i < points.length - 1; i++) {
			const prev = points[i - 1];
			const curr = points[i];
			const next = points[i + 1];

			const dPrev = Math.hypot(curr.x - prev.x, curr.y - prev.y);
			const dNext = Math.hypot(next.x - curr.x, next.y - curr.y);

			if (dPrev < 0.1 || dNext < 0.1) {
				path += ` L ${curr.x} ${curr.y}`;
				continue;
			}

			const r = Math.min(radius, dPrev / 2, dNext / 2);

			if (r <= 0.1) {
				path += ` L ${curr.x} ${curr.y}`;
				continue;
			}

			const startX = curr.x - (curr.x - prev.x) * (r / dPrev);
			const startY = curr.y - (curr.y - prev.y) * (r / dPrev);

			const endX = curr.x + (next.x - curr.x) * (r / dNext);
			const endY = curr.y + (next.y - curr.y) * (r / dNext);

			path += ` L ${startX} ${startY} Q ${curr.x} ${curr.y} ${endX} ${endY}`;
		}
		path += ` L ${points[points.length - 1].x} ${points[points.length - 1].y}`;
		return path;
	};

	const findOrthogonalRoute = (
		start: GraphNodePosition,
		end: GraphNodePosition
	): GraphNodePosition[] | null => {
		const grid = ROUTING_GRID_REM * remPx;
		const margin = ROUTING_MARGIN_REM * remPx;
		const sourceX = Math.ceil(start.x / grid);
		const sourceY = Math.round(start.y / grid);
		const targetX = Math.floor(end.x / grid);
		const targetY = Math.round(end.y / grid);
		const minX = Math.floor((Math.min(start.x, end.x) - margin) / grid);
		const maxX = Math.ceil((Math.max(start.x, end.x) + margin) / grid);
		const minY = Math.floor((Math.min(start.y, end.y) - margin) / grid);
		const maxY = Math.ceil((Math.max(start.y, end.y) + margin) / grid);
		const directions = [
			{ x: 1, y: 0 },
			{ x: 0, y: 1 },
			{ x: -1, y: 0 },
			{ x: 0, y: -1 }
		];
		const open: RoutingState[] = [];
		const costs = new Map<string, number>();
		const parents = new Map<string, string>();
		const states = new Map<string, RoutingState>();
		const startState: RoutingState = {
			x: sourceX,
			y: sourceY,
			direction: -1,
			cost: 0,
			estimate: Math.abs(targetX - sourceX) + Math.abs(targetY - sourceY),
			key: routingStateKey(sourceX, sourceY, -1)
		};
		costs.set(startState.key, 0);
		states.set(startState.key, startState);
		heapPush(open, startState);

		let visits = 0;
		let goal: RoutingState | null = null;
		while (open.length > 0 && visits < ROUTING_MAX_VISITS) {
			const current = heapPop(open);
			if (!current || current.cost !== costs.get(current.key)) {
				continue;
			}
			visits += 1;
			if (current.x === targetX && current.y === targetY) {
				goal = current;
				break;
			}
			for (let direction = 0; direction < directions.length; direction += 1) {
				const nextX = current.x + directions[direction].x;
				const nextY = current.y + directions[direction].y;
				if (nextX < minX || nextX > maxX || nextY < minY || nextY > maxY) {
					continue;
				}
				if (routingPointBlocked(nextX * grid, nextY * grid)) {
					continue;
				}
				const turnCost =
					current.direction >= 0 && current.direction !== direction ? ROUTING_TURN_COST : 0;
				const nextCost = current.cost + 1 + turnCost;
				const key = routingStateKey(nextX, nextY, direction);
				if (nextCost >= (costs.get(key) ?? Number.POSITIVE_INFINITY)) {
					continue;
				}
				const next: RoutingState = {
					x: nextX,
					y: nextY,
					direction,
					cost: nextCost,
					estimate: nextCost + Math.abs(targetX - nextX) + Math.abs(targetY - nextY),
					key
				};
				costs.set(key, nextCost);
				parents.set(key, current.key);
				states.set(key, next);
				heapPush(open, next);
			}
		}
		if (!goal) {
			return null;
		}
		const points: GraphNodePosition[] = [];
		let key: string | undefined = goal.key;
		while (key) {
			const state = states.get(key);
			if (!state) {
				break;
			}
			points.push({ x: state.x * grid, y: state.y * grid });
			key = parents.get(key);
		}
		points.reverse();
		return simplifyRoute(points);
	};

	const routedWirePath = (
		edge: GraphEdge,
		start: GraphNodePosition,
		end: GraphNodePosition
	): string | null => {
		const sourceNode = nodesById.get(edge.from.nodeId);
		const targetNode = nodesById.get(edge.to.nodeId);
		if (!sourceNode || !targetNode || sourceNode.id === targetNode.id) {
			return null;
		}
		const grid = ROUTING_GRID_REM * remPx;
		const clearance = ROUTING_CLEARANCE_REM * remPx;
		const routeStart = {
			x:
				Math.ceil(((sourceNode.position.x + nodeWidth(sourceNode)) * remPx + clearance) / grid) *
					grid +
				grid,
			y: start.y
		};
		const routeEnd = {
			x: Math.floor((targetNode.position.x * remPx - clearance) / grid) * grid - grid,
			y: end.y
		};
		const gridStart = { x: routeStart.x, y: Math.round(routeStart.y / grid) * grid };
		const gridEnd = { x: routeEnd.x, y: Math.round(routeEnd.y / grid) * grid };
		const middle = findOrthogonalRoute(gridStart, gridEnd);
		if (!middle) {
			return null;
		}

		let points = simplifyRoute([routeStart, gridStart, ...middle, gridEnd, routeEnd]);
		points = smoothRoute(points);
		points = simplifyRoute([start, ...points, end]);

		return roundedPath(points, remPx * 1.5);
	};

	const edgePath = (edge: GraphEdge): string | null => {
		const start = socketPoint(edge.from, 'output');
		const end = socketPoint(edge.to, 'input');
		if (!start || !end) {
			return null;
		}
		if (!routeEdgesAroundNodes) {
			return wirePath(start, end);
		}
		const cacheKey = `${edge.id ?? ''}:${edge.from.nodeId}:${edge.from.socketId}:${edge.to.nodeId}:${edge.to.socketId}`;
		const cached = routedPathCache.get(cacheKey);
		if (cached) {
			return cached;
		}
		const path = routedWirePath(edge, start, end) ?? wirePath(start, end);
		routedPathCache.set(cacheKey, path);
		return path;
	};

	const edgeUsesGradient = (edge: GraphEdge): boolean =>
		edge.color !== undefined && edge.targetColor !== undefined && edge.color !== edge.targetColor;

	const edgeGradientId = (edge: GraphEdge, index: number): string =>
		`edge-gradient-${(edge.id ?? `${edge.from.nodeId}-${edge.from.socketId}-${edge.to.nodeId}-${edge.to.socketId}-${index}`).replace(/[^a-zA-Z0-9_-]/g, '_')}`;

	const edgeStroke = (edge: GraphEdge, gradientId: string): string | undefined =>
		edgeUsesGradient(edge) ? `url(#${gradientId})` : edge.color;

	const draftPath = (): string | null => {
		if (!connectionDraft) {
			return null;
		}
		const start = socketPoint(connectionDraft.from, 'output');
		if (!start) return null;

		if (connectionDraft.snapNodeId && connectionDraft.snapSocketId) {
			const end = socketPoint(
				{ nodeId: connectionDraft.snapNodeId, socketId: connectionDraft.snapSocketId },
				'input'
			);
			if (end) return wirePath(start, end);
		}

		return wirePath(start, { x: connectionDraft.endX, y: connectionDraft.endY });
	};

	const clientToWorldPx = (clientX: number, clientY: number): { x: number; y: number } => {
		const bounds = container?.getBoundingClientRect();
		return {
			x: (clientX - (bounds?.left ?? 0) - camera.x) / camera.zoom,
			y: (clientY - (bounds?.top ?? 0) - camera.y) / camera.zoom
		};
	};

	const cameraAtZoom = (zoom: number, anchorX: number, anchorY: number): GraphCamera => {
		const nextZoom = clamp(zoom, effectiveMinZoom, effectiveMaxZoom);
		const worldX = (anchorX - camera.x) / camera.zoom;
		const worldY = (anchorY - camera.y) / camera.zoom;
		return {
			x: anchorX - worldX * nextZoom,
			y: anchorY - worldY * nextZoom,
			zoom: nextZoom
		};
	};

	const cancelCameraAnimation = (): void => {
		if (animationFrame !== null) {
			cancelAnimationFrame(animationFrame);
			animationFrame = null;
		}
	};

	$effect(() => {
		if (!initialCamera) {
			return;
		}
		const normalized = normalizeCamera(initialCamera);
		const key = cameraKey(normalized);
		if (key === appliedInitialCameraKey) {
			return;
		}
		appliedInitialCameraKey = key;
		if (!camerasMatch(camera, normalized)) {
			cancelCameraAnimation();
			applyCamera(normalized, false);
		}
	});

	const animateCamera = (target: GraphCamera): void => {
		cancelCameraAnimation();
		const source = { ...camera };
		const startedAt = performance.now();
		const tick = (now: number): void => {
			const progress = clamp((now - startedAt) / CAMERA_ANIMATION_MS, 0, 1);
			const eased = 1 - (1 - progress) ** 3;
			applyCamera({
				x: source.x + (target.x - source.x) * eased,
				y: source.y + (target.y - source.y) * eased,
				zoom: source.zoom + (target.zoom - source.zoom) * eased
			});
			if (progress < 1) {
				animationFrame = requestAnimationFrame(tick);
			} else {
				animationFrame = null;
			}
		};
		animationFrame = requestAnimationFrame(tick);
	};

	const setZoom = (zoom: number): void => {
		const center = visibleViewportCenter();
		cancelCameraAnimation();
		applyCamera(cameraAtZoom(zoom, center.x, center.y));
	};

	const resetZoom = (): void => {
		const center = visibleViewportCenter();
		animateCamera(cameraAtZoom(1, center.x, center.y));
	};

	const handleZoomInput = (event: Event): void => {
		setZoom((event.currentTarget as HTMLInputElement).valueAsNumber / 100);
	};

	const frameBounds = (bounds: GraphWorldBounds | null): boolean => {
		if (!container || bounds === null) {
			return false;
		}
		const left = Math.min(bounds.left, bounds.right);
		const top = Math.min(bounds.top, bounds.bottom);
		const right = Math.max(bounds.left, bounds.right);
		const bottom = Math.max(bounds.top, bounds.bottom);
		if (![left, top, right, bottom].every(Number.isFinite)) {
			return false;
		}
		const widthPx = Math.max(remPx, (right - left) * remPx);
		const heightPx = Math.max(remPx, (bottom - top) * remPx);
		const paddingPx = FRAME_PADDING_REM * remPx;
		const viewport = visibleViewport();
		const zoom = clamp(
			Math.min(
				(viewport.width - paddingPx * 2) / widthPx,
				(viewport.height - paddingPx * 2) / heightPx
			),
			effectiveMinZoom,
			effectiveMaxZoom
		);
		const centerX = (left + right) * 0.5 * remPx;
		const centerY = (top + bottom) * 0.5 * remPx;
		const viewportCenter = {
			x: viewport.x + viewport.width * 0.5,
			y: viewport.y + viewport.height * 0.5
		};
		animateCamera({
			x: viewportCenter.x - centerX * zoom,
			y: viewportCenter.y - centerY * zoom,
			zoom
		});
		return true;
	};

	const boundsForNodes = (candidates: GraphNode[]): GraphWorldBounds | null => {
		if (candidates.length === 0) {
			return null;
		}
		return {
			left: Math.min(...candidates.map((node) => node.position.x)),
			top: Math.min(...candidates.map((node) => node.position.y)),
			right: Math.max(...candidates.map((node) => node.position.x + nodeWidth(node))),
			bottom: Math.max(...candidates.map((node) => node.position.y + nodeHeight(node)))
		};
	};

	const frameNodes = (candidates: GraphNode[]): boolean => frameBounds(boundsForNodes(candidates));

	export const frameSelection = (): boolean => {
		const selected = effectiveNodes.filter((node) => selectedIds.has(node.id));
		if (selected.length > 0 || effectiveNodes.length > 0) {
			return frameNodes(selected.length > 0 ? selected : effectiveNodes);
		}
		return frameBounds(worldBounds ?? null);
	};

	export const home = (): boolean =>
		effectiveNodes.length > 0 ? frameNodes(effectiveNodes) : frameBounds(worldBounds ?? null);

	export const focus = (): void => {
		container?.focus();
	};

	export const clientToWorld = (clientX: number, clientY: number): GraphNodePosition => {
		const world = clientToWorldPx(clientX, clientY);
		return {
			x: world.x / remPx,
			y: world.y / remPx
		};
	};

	export const viewportCenter = (): GraphNodePosition => {
		const center = visibleViewportCenter();
		return {
			x: (center.x - camera.x) / camera.zoom / remPx,
			y: (center.y - camera.y) / camera.zoom / remPx
		};
	};

	const publishSelection = (nodeIds: string[], edgeIds: string[]): void => {
		onGraphSelectionChange?.(nodeIds, edgeIds);
	};

	const updateSelection = (nodeId: string, additive: boolean): void => {
		const next = additive ? new Set(selectedIds) : new Set<string>();
		if (additive && next.has(nodeId)) {
			next.delete(nodeId);
		} else {
			next.add(nodeId);
		}
		publishSelection([...next], additive ? [...selectedEdgeIdSet] : []);
	};

	const updateEdgeSelection = (edge: GraphEdge, additive: boolean): void => {
		if (!edge.id) {
			return;
		}
		const next = additive ? new Set(selectedEdgeIdSet) : new Set<string>();
		if (additive && next.has(edge.id)) {
			next.delete(edge.id);
		} else {
			next.add(edge.id);
		}
		publishSelection(additive ? [...selectedIds] : [], [...next]);
	};

	const selectEdge = (event: PointerEvent, edge: GraphEdge): void => {
		if (event.button !== 0) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		container?.focus();
		updateEdgeSelection(edge, event.ctrlKey || event.metaKey || event.shiftKey);
	};

	const selectEdgeWithKeyboard = (event: KeyboardEvent, edge: GraphEdge): void => {
		if (event.key !== 'Enter' && event.key !== ' ') {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		updateEdgeSelection(edge, event.ctrlKey || event.metaKey || event.shiftKey);
	};

	const localPointerPosition = (event: PointerEvent): { x: number; y: number } => {
		const bounds = container?.getBoundingClientRect();
		return {
			x: event.clientX - (bounds?.left ?? 0),
			y: event.clientY - (bounds?.top ?? 0)
		};
	};

	const finishSelectionGesture = (): void => {
		if (!selectionGesture) {
			return;
		}
		const left = Math.min(selectionGesture.startX, selectionGesture.currentX);
		const top = Math.min(selectionGesture.startY, selectionGesture.currentY);
		const right = Math.max(selectionGesture.startX, selectionGesture.currentX);
		const bottom = Math.max(selectionGesture.startY, selectionGesture.currentY);
		if (right - left < 3 && bottom - top < 3) {
			selectionGesture = null;
			return;
		}
		const next = new Set(selectionGesture.initialSelectedNodeIds);
		for (const node of effectiveNodes) {
			const nodeLeft = camera.x + node.position.x * remPx * camera.zoom;
			const nodeTop = camera.y + node.position.y * remPx * camera.zoom;
			const nodeRight = nodeLeft + nodeWidth(node) * remPx * camera.zoom;
			const nodeBottom = nodeTop + nodeHeight(node) * remPx * camera.zoom;
			if (nodeRight >= left && nodeLeft <= right && nodeBottom >= top && nodeTop <= bottom) {
				next.add(node.id);
			}
		}
		publishSelection([...next], [...selectedEdgeIdSet]);
		selectionGesture = null;
	};

	const startPan = (event: PointerEvent): void => {
		if (event.button !== 0 && event.button !== 1) {
			return;
		}
		container?.focus();
		if (event.button === 0 && event.shiftKey) {
			const pointer = localPointerPosition(event);
			selectionGesture = {
				pointerId: event.pointerId,
				startX: pointer.x,
				startY: pointer.y,
				currentX: pointer.x,
				currentY: pointer.y,
				initialSelectedNodeIds: [...selectedIds]
			};
			container?.setPointerCapture(event.pointerId);
			return;
		}
		panGesture = {
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			cameraX: camera.x,
			cameraY: camera.y,
			moved: false,
			clearSelectionOnClick: event.button === 0
		};
		container?.setPointerCapture(event.pointerId);
	};

	const selectNodeBody = (event: PointerEvent, node: GraphNode): void => {
		if (event.button !== 0) {
			return;
		}
		event.stopPropagation();
		container?.focus();
		const target = event.target;
		if (event.altKey && target instanceof Element && target.closest('.node-header')) {
			event.preventDefault();
			toggleNodeCollapsed(node);
			return;
		}
		if (
			target instanceof Element &&
			target.closest('button, input, textarea, select, a, [data-no-node-select]')
		) {
			return;
		}
		if (event.ctrlKey || event.metaKey || event.shiftKey) {
			updateSelection(node.id, true);
		} else if (!selectedIds.has(node.id)) {
			updateSelection(node.id, false);
		}
	};

	const startNodeDrag = (event: PointerEvent, node: GraphNode): void => {
		if (event.button !== 0) {
			return;
		}
		event.stopPropagation();
		container?.focus();
		const target = event.target;
		if (event.altKey && target instanceof Element && target.closest('.node-header')) {
			event.preventDefault();
			toggleNodeCollapsed(node);
			return;
		}
		const additive = event.ctrlKey || event.metaKey || event.shiftKey;
		let dragIds: Set<string>;
		if (additive) {
			const next = new Set(selectedIds);
			if (next.has(node.id)) {
				next.delete(node.id);
			} else {
				next.add(node.id);
			}
			publishSelection([...next], additive ? [...selectedEdgeIdSet] : []);
			if (!next.has(node.id)) {
				return;
			}
			dragIds = next;
		} else if (selectedIds.has(node.id)) {
			dragIds = new Set(selectedIds);
		} else {
			updateSelection(node.id, false);
			dragIds = new Set([node.id]);
		}
		const nodePositions: Record<string, GraphNodePosition> = {};
		for (const dragId of dragIds) {
			const dragNode = nodesById.get(dragId);
			if (dragNode) {
				nodePositions[dragId] = dragNode.position;
			}
		}
		nodeDragGesture = {
			pointerId: event.pointerId,
			startX: event.clientX,
			startY: event.clientY,
			nodePositions
		};
		container?.setPointerCapture(event.pointerId);
	};

	const startNodeResize = (event: PointerEvent, node: GraphNode): void => {
		if (event.button !== 0 || !node.resizable) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		container?.focus();
		const now = Date.now();
		const previousPointerDown = lastResizePointerDown;
		lastResizePointerDown = {
			nodeId: node.id,
			time: now,
			x: event.clientX,
			y: event.clientY
		};
		if (
			previousPointerDown?.nodeId === node.id &&
			now - previousPointerDown.time <= TITLE_DOUBLE_CLICK_MS &&
			Math.hypot(event.clientX - previousPointerDown.x, event.clientY - previousPointerDown.y) <=
				TITLE_DOUBLE_CLICK_DISTANCE_PX
		) {
			lastResizePointerDown = null;
			if (!selectedIds.has(node.id)) {
				updateSelection(node.id, false);
			}
			commitNodeResize({ nodeId: node.id, mode: 'automatic' });
			return;
		}
		if (!selectedIds.has(node.id)) {
			updateSelection(node.id, false);
		}
		nodeResizeGesture = {
			pointerId: event.pointerId,
			nodeId: node.id,
			startX: event.clientX,
			startY: event.clientY,
			startSize: {
				width: nodeWidth(node),
				height: nodeHeight(node)
			}
		};
		container?.setPointerCapture(event.pointerId);
	};

	const startConnection = (event: PointerEvent, nodeId: string, socket: GraphSocket): void => {
		const node = nodesById.get(nodeId);
		if (event.button !== 0 || !node || socketDisabled(node, socket, 'output')) {
			return;
		}
		event.stopPropagation();
		const point = socketPoint({ nodeId, socketId: socket.id }, 'output');
		if (!point) {
			return;
		}
		connectionDraft = {
			pointerId: event.pointerId,
			from: { nodeId, socketId: socket.id },
			endX: point.x,
			endY: point.y
		};
		container?.setPointerCapture(event.pointerId);
	};

	const completeConnection = (nodeId: string, socket: GraphSocket): boolean => {
		const node = nodesById.get(nodeId);
		if (!connectionDraft || !node || socketDisabled(node, socket, 'input')) {
			return false;
		}
		const connection = {
			from: connectionDraft.from,
			to: { nodeId, socketId: socket.id }
		};
		if (!connectionAllowed(connection)) {
			return false;
		}
		onConnect?.(connection);
		connectionDraft = null;
		return true;
	};

	const finishConnection = (event: PointerEvent, nodeId: string, socket: GraphSocket): void => {
		if (completeConnection(nodeId, socket)) {
			event.stopPropagation();
		}
	};

	const findInputSocket = (
		node: GraphNode | undefined,
		socketId: string
	): GraphSocket | undefined =>
		node ? allNodeSockets(node, 'input').find((socket) => socket.id === socketId) : undefined;

	const finishConnectionAtPointer = (event: PointerEvent): void => {
		if (connectionDraft?.snapNodeId && connectionDraft?.snapSocketId) {
			const socket = findInputSocket(
				nodesById.get(connectionDraft.snapNodeId),
				connectionDraft.snapSocketId
			);
			if (socket) {
				completeConnection(connectionDraft.snapNodeId, socket);
				return;
			}
		}

		const target = document.elementFromPoint(event.clientX, event.clientY);
		const inputSocket =
			target instanceof Element
				? target.closest<HTMLButtonElement>('.socket.input[data-node-id][data-socket-id]')
				: null;
		const nodeId = inputSocket?.dataset.nodeId;
		const socketId = inputSocket?.dataset.socketId;
		if (!nodeId || !socketId) {
			return;
		}
		const socket = findInputSocket(nodesById.get(nodeId), socketId);
		if (socket) {
			completeConnection(nodeId, socket);
		}
	};

	const handlePointerMove = (event: PointerEvent): void => {
		const hasActiveGesture =
			selectionGesture?.pointerId === event.pointerId ||
			panGesture?.pointerId === event.pointerId ||
			nodeDragGesture?.pointerId === event.pointerId ||
			nodeResizeGesture?.pointerId === event.pointerId ||
			connectionDraft?.pointerId === event.pointerId;
		if (!hasActiveGesture) {
			updateScrollHoverTarget(event);
		}
		if (selectionGesture?.pointerId === event.pointerId) {
			const pointer = localPointerPosition(event);
			selectionGesture = {
				...selectionGesture,
				currentX: pointer.x,
				currentY: pointer.y
			};
			return;
		}
		if (panGesture?.pointerId === event.pointerId) {
			const deltaX = event.clientX - panGesture.startX;
			const deltaY = event.clientY - panGesture.startY;
			if (!panGesture.moved && Math.hypot(deltaX, deltaY) >= 3) {
				panGesture = { ...panGesture, moved: true };
			}
			applyCamera({
				...camera,
				x: panGesture.cameraX + deltaX,
				y: panGesture.cameraY + deltaY
			});
			return;
		}
		if (nodeDragGesture?.pointerId === event.pointerId) {
			const deltaX = (event.clientX - nodeDragGesture.startX) / camera.zoom / remPx;
			const deltaY = (event.clientY - nodeDragGesture.startY) / camera.zoom / remPx;
			const nextPositions: Record<string, GraphNodePosition> = {};
			for (const [nodeId, position] of Object.entries(nodeDragGesture.nodePositions)) {
				nextPositions[nodeId] = {
					x: position.x + deltaX,
					y: position.y + deltaY
				};
			}
			dragPositions = nextPositions;
			return;
		}
		if (nodeResizeGesture?.pointerId === event.pointerId) {
			const node = nodesById.get(nodeResizeGesture.nodeId);
			if (!node) {
				return;
			}
			const deltaX = (event.clientX - nodeResizeGesture.startX) / camera.zoom / remPx;
			const deltaY = (event.clientY - nodeResizeGesture.startY) / camera.zoom / remPx;
			resizeSizes = {
				...resizeSizes,
				[node.id]: {
					width: Math.max(MIN_NODE_WIDTH_REM, nodeResizeGesture.startSize.width + deltaX),
					height: Math.max(minimumNodeHeight(node), nodeResizeGesture.startSize.height + deltaY)
				}
			};
			return;
		}
		if (connectionDraft?.pointerId === event.pointerId) {
			const world = clientToWorldPx(event.clientX, event.clientY);

			let bestSnapDist = remPx * 4; // max snap distance
			let bestNodeId: string | undefined = undefined;
			let bestSocketId: string | undefined = undefined;

			for (const node of effectiveNodes) {
				if (node.id === connectionDraft.from.nodeId) continue;

				for (const socket of displayNodeSockets(node, 'input')) {
					if (socketDisabled(node, socket, 'input')) continue;
					const pt = socketPoint({ nodeId: node.id, socketId: socket.id }, 'input');
					if (!pt) continue;

					const dist = Math.hypot(world.x - pt.x, world.y - pt.y);
					if (dist < bestSnapDist) {
						bestSnapDist = dist;
						bestNodeId = node.id;
						bestSocketId = socket.id;
					}
				}

				// Snap to node header if close to the node bounds and it has exactly one compatible input
				if (node.inputs.length === 1 && !socketDisabled(node, node.inputs[0], 'input')) {
					const left = node.position.x * remPx;
					const top = node.position.y * remPx;
					const right = left + nodeWidth(node) * remPx;
					const bottom = top + nodeHeight(node) * remPx;

					// If inside node bounds or slightly expanded bounds
					const margin = remPx * 1;
					if (
						world.x >= left - margin &&
						world.x <= right + margin &&
						world.y >= top - margin &&
						world.y <= bottom + margin
					) {
						// Only snap to the whole node if it's better than our current best distance
						// Being inside the node bounds is considered distance 0
						if (bestSnapDist > 0) {
							bestSnapDist = 0; // Prioritize this since we are directly over the node
							bestNodeId = node.id;
							bestSocketId = node.inputs[0].id;
						}
					}
				}
			}

			connectionDraft = {
				...connectionDraft,
				endX: world.x,
				endY: world.y,
				snapNodeId: bestNodeId,
				snapSocketId: bestSocketId
			};
		}
	};

	const clearOptimisticMoves = (moves: GraphNodeMove[]): void => {
		const next = { ...optimisticPositions };
		let changed = false;
		for (const move of moves) {
			const pending = next[move.nodeId];
			if (pending?.x === move.position.x && pending?.y === move.position.y) {
				delete next[move.nodeId];
				changed = true;
			}
		}
		if (changed) {
			optimisticPositions = next;
		}
	};

	const commitNodeMoves = (moves: GraphNodeMove[]): void => {
		optimisticPositions = {
			...optimisticPositions,
			...Object.fromEntries(moves.map((move) => [move.nodeId, move.position]))
		};
		try {
			const result = onNodesMove
				? onNodesMove(moves)
				: Promise.all(moves.map((move) => onNodeMove?.(move.nodeId, move.position)));
			void Promise.resolve(result).catch(() => clearOptimisticMoves(moves));
		} catch {
			clearOptimisticMoves(moves);
		}
	};

	const clearOptimisticResize = (resize: GraphNodeResize): void => {
		const pending = optimisticSizes[resize.nodeId];
		const matches =
			resize.mode === 'automatic'
				? pending === null
				: pending?.width === resize.size.width && pending?.height === resize.size.height;
		if (matches) {
			const next = { ...optimisticSizes };
			delete next[resize.nodeId];
			optimisticSizes = next;
		}
	};

	const commitNodeResize = (resize: GraphNodeResize): void => {
		optimisticSizes = {
			...optimisticSizes,
			[resize.nodeId]: resize.mode === 'custom' ? resize.size : null
		};
		try {
			void Promise.resolve(onNodeResize?.(resize)).catch(() => clearOptimisticResize(resize));
		} catch {
			clearOptimisticResize(resize);
		}
	};

	const resetNodeSize = (event: MouseEvent, node: GraphNode): void => {
		event.preventDefault();
		event.stopPropagation();
		if (!node.resizable) {
			return;
		}
		commitNodeResize({ nodeId: node.id, mode: 'automatic' });
	};

	const clearOptimisticRename = (nodeId: string, label: string): void => {
		if (optimisticLabels[nodeId] !== label) {
			return;
		}
		const next = { ...optimisticLabels };
		delete next[nodeId];
		optimisticLabels = next;
	};

	const clearOptimisticCollapse = (nodeId: string, collapsed: boolean): void => {
		if (optimisticCollapsed[nodeId] !== collapsed) {
			return;
		}
		const next = { ...optimisticCollapsed };
		delete next[nodeId];
		optimisticCollapsed = next;
	};

	const clearOptimisticEnabled = (nodeId: string, enabled: boolean): void => {
		if (optimisticEnabled[nodeId] !== enabled) {
			return;
		}
		const next = { ...optimisticEnabled };
		delete next[nodeId];
		optimisticEnabled = next;
	};

	const toggleNodeCollapsed = (node: GraphNode): void => {
		const collapsed = node.collapsed !== true;
		optimisticCollapsed = { ...optimisticCollapsed, [node.id]: collapsed };
		try {
			void Promise.resolve(onNodeCollapsedChange?.(node.id, collapsed)).catch(() =>
				clearOptimisticCollapse(node.id, collapsed)
			);
		} catch {
			clearOptimisticCollapse(node.id, collapsed);
		}
	};

	const canToggleNodeEnabled = (node: GraphNode): boolean =>
		onNodeEnabledChange !== undefined && node.canDisable === true;

	const toggleNodeEnabled = (event: MouseEvent, node: GraphNode): void => {
		event.preventDefault();
		event.stopPropagation();
		if (!canToggleNodeEnabled(node)) {
			return;
		}
		const enabled = node.enabled === false;
		optimisticEnabled = { ...optimisticEnabled, [node.id]: enabled };
		try {
			void Promise.resolve(onNodeEnabledChange?.(node.id, enabled)).catch(() =>
				clearOptimisticEnabled(node.id, enabled)
			);
		} catch {
			clearOptimisticEnabled(node.id, enabled);
		}
	};

	const canRenameNode = (node: GraphNode): boolean =>
		onNodeRename !== undefined && node.canRename !== false;

	const isPurePrimaryClick = (event: MouseEvent | PointerEvent): boolean =>
		event.button === 0 && !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;

	const beginNodeRename = (event: MouseEvent, node: GraphNode): void => {
		if (!canRenameNode(node) || !isPurePrimaryClick(event)) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		if (!selectedIds.has(node.id)) {
			updateSelection(node.id, false);
		}
		renamingNodeId = node.id;
		renameDraft = node.label;
		focusedRenameNodeId = null;
	};

	const handleNodeTitlePointerDown = (event: PointerEvent, node: GraphNode): void => {
		if (!isPurePrimaryClick(event)) {
			lastTitlePointerDown = null;
			startNodeDrag(event, node);
			return;
		}
		const elapsed = lastTitlePointerDown ? event.timeStamp - lastTitlePointerDown.time : Infinity;
		const distance = lastTitlePointerDown
			? Math.hypot(event.clientX - lastTitlePointerDown.x, event.clientY - lastTitlePointerDown.y)
			: Infinity;
		const doubleClick =
			lastTitlePointerDown?.nodeId === node.id &&
			elapsed >= 0 &&
			elapsed <= TITLE_DOUBLE_CLICK_MS &&
			distance <= TITLE_DOUBLE_CLICK_DISTANCE_PX;
		if (doubleClick && canRenameNode(node)) {
			lastTitlePointerDown = null;
			beginNodeRename(event, node);
			return;
		}
		lastTitlePointerDown = {
			nodeId: node.id,
			time: event.timeStamp,
			x: event.clientX,
			y: event.clientY
		};
		startNodeDrag(event, node);
	};

	const finishNodeRename = (): void => {
		const nodeId = renamingNodeId;
		if (!nodeId) {
			return;
		}
		const node = nodes.find((candidate) => candidate.id === nodeId);
		const label = renameDraft.trim();
		renamingNodeId = null;
		renameInput = null;
		focusedRenameNodeId = null;
		if (!node || !label || label === node.label) {
			return;
		}
		optimisticLabels = { ...optimisticLabels, [nodeId]: label };
		try {
			void Promise.resolve(onNodeRename?.(nodeId, label)).catch(() =>
				clearOptimisticRename(nodeId, label)
			);
		} catch {
			clearOptimisticRename(nodeId, label);
		}
	};

	const cancelNodeRename = (): void => {
		renamingNodeId = null;
		renameInput = null;
		focusedRenameNodeId = null;
	};

	const handleRenameKeydown = (event: KeyboardEvent): void => {
		if (event.key === 'Enter') {
			event.preventDefault();
			finishNodeRename();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			cancelNodeRename();
		}
	};

	const handlePointerEnd = (event: PointerEvent): void => {
		if (selectionGesture?.pointerId === event.pointerId) {
			finishSelectionGesture();
		}
		if (panGesture?.pointerId === event.pointerId) {
			if (!panGesture.moved && panGesture.clearSelectionOnClick) {
				publishSelection([], []);
			}
			panGesture = null;
		}
		if (nodeDragGesture?.pointerId === event.pointerId) {
			const moves = Object.entries(dragPositions).map(
				([nodeId, position]): GraphNodeMove => ({ nodeId, position })
			);
			if (moves.length > 0) {
				commitNodeMoves(moves);
			}
			dragPositions = {};
			nodeDragGesture = null;
		}
		if (nodeResizeGesture?.pointerId === event.pointerId) {
			const size = resizeSizes[nodeResizeGesture.nodeId];
			if (size) {
				commitNodeResize({
					nodeId: nodeResizeGesture.nodeId,
					mode: 'custom',
					size
				});
			}
			resizeSizes = {};
			nodeResizeGesture = null;
		}
		if (connectionDraft?.pointerId === event.pointerId) {
			finishConnectionAtPointer(event);
			connectionDraft = null;
		}
		if (container?.hasPointerCapture(event.pointerId)) {
			container.releasePointerCapture(event.pointerId);
		}
	};

	const isScrollableElement = (element: HTMLElement): boolean => {
		if (element.scrollHeight <= element.clientHeight) {
			return false;
		}
		const overflowY = getComputedStyle(element).overflowY;
		return overflowY === 'auto' || overflowY === 'scroll';
	};

	const findScrollableUnderPointer = (target: EventTarget | null): HTMLElement | null => {
		let element = target instanceof HTMLElement ? target : null;
		while (element && element !== container) {
			if (isScrollableElement(element)) {
				return element;
			}
			element = element.parentElement;
		}
		return null;
	};

	const clearScrollActivationTimer = (): void => {
		if (scrollActivationTimer !== null) {
			clearTimeout(scrollActivationTimer);
			scrollActivationTimer = null;
		}
	};

	// Briefly passing the pointer over a scrollable area keeps wheel = zoom; only once the
	// pointer dwells there past SCROLL_TARGET_ACTIVATION_MS does wheel switch to scrolling it.
	const updateScrollHoverTarget = (event: PointerEvent): void => {
		const candidate = findScrollableUnderPointer(event.target);
		if (candidate === hoveredScrollTarget) {
			return;
		}
		hoveredScrollTarget = candidate;
		wheelScrollTarget = null;
		clearScrollActivationTimer();
		if (candidate) {
			scrollActivationTimer = setTimeout(() => {
				wheelScrollTarget = candidate;
				scrollActivationTimer = null;
			}, SCROLL_TARGET_ACTIVATION_MS);
		}
	};

	const resetScrollHoverTarget = (): void => {
		hoveredScrollTarget = null;
		wheelScrollTarget = null;
		clearScrollActivationTimer();
	};

	const handleWheel = (event: WheelEvent): void => {
		if (
			wheelScrollTarget &&
			event.target instanceof Node &&
			wheelScrollTarget.contains(event.target)
		) {
			return;
		}
		event.preventDefault();
		const bounds = container?.getBoundingClientRect();
		if (!bounds) {
			return;
		}
		const pointerX = event.clientX - bounds.left;
		const pointerY = event.clientY - bounds.top;
		cancelCameraAnimation();
		applyCamera(cameraAtZoom(camera.zoom * Math.exp(-event.deltaY * 0.0014), pointerX, pointerY));
	};

	const handleContextMenu = (event: MouseEvent): void => {
		const targetElement = event.target instanceof Element ? event.target : null;
		const nodeTarget = targetElement?.closest<HTMLElement>('[data-node-id]');
		if (nodeTarget) {
			event.preventDefault();
			event.stopPropagation();
			const nodeId = nodeTarget.dataset.nodeId;
			if (nodeId) {
				onNodeContextMenu?.(event, nodeId);
			}
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		if (!onBackgroundContextMenu || targetElement?.closest('.toolbar')) {
			return;
		}
		onBackgroundContextMenu?.(event, clientToWorld(event.clientX, event.clientY));
	};

	const handleDoubleClick = (event: MouseEvent): void => {
		const targetElement = event.target instanceof Element ? event.target : null;
		if (
			!onBackgroundDoubleClick ||
			targetElement?.closest(
				'.toolbar, [data-node-id], button, input, select, textarea, a, [role="button"]'
			)
		) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		onBackgroundDoubleClick(event, clientToWorld(event.clientX, event.clientY));
	};

	const handleKeydown = (event: KeyboardEvent): void => {
		if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) {
			return;
		}
		const target = event.target;
		if (
			target instanceof HTMLInputElement ||
			target instanceof HTMLSelectElement ||
			target instanceof HTMLTextAreaElement ||
			(target instanceof HTMLElement && target.isContentEditable)
		) {
			return;
		}
		if (event.key.toLowerCase() === 'f') {
			event.preventDefault();
			event.stopPropagation();
			frameSelection();
		} else if (event.key.toLowerCase() === 'h') {
			event.preventDefault();
			event.stopPropagation();
			home();
		} else if (event.key === ' ' && onCreateRequest && container) {
			event.preventDefault();
			event.stopPropagation();
			const bounds = container.getBoundingClientRect();
			onCreateRequest({
				position: viewportCenter(),
				clientX: bounds.left + bounds.width * 0.5,
				clientY: bounds.top + bounds.height * 0.5
			});
		}
	};

	onMount(() => {
		if (!container) {
			return;
		}
		remPx = Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;
		const observer = new ResizeObserver(([entry]) => {
			viewportWidth = Math.max(1, entry.contentRect.width);
			viewportHeight = Math.max(1, entry.contentRect.height);
		});
		observer.observe(container);
		if (autoHomeOnMount && !initialCamera) {
			requestAnimationFrame(() => home());
		}
		return () => {
			observer.disconnect();
			cancelCameraAnimation();
			clearScrollActivationTimer();
		};
	});
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
	bind:this={container}
	class:panning={panGesture !== null}
	class:node-dragging={nodeDragGesture !== null}
	class:node-resizing={nodeResizeGesture !== null}
	class:selecting={selectionGesture !== null}
	class:connecting={connectionDraft !== null}
	class="graph-canvas"
	data-socket-labels={socketLabels}
	data-local-context-menu={onBackgroundContextMenu ? '' : undefined}
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
					{@render worldContent({ camera, remPx, viewportWidth, viewportHeight })}
				</div>
			{/if}

			<svg class="wires" aria-label="Graph connections">
				{#each visibleEdges as edge, index (`${edge.id ?? index}:${edge.from.nodeId}:${edge.to.nodeId}`)}
					{@const path = edgePath(edge)}
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
				<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
				<article
					class:selected={selectedIds.has(node.id)}
					class:active={node.active}
					class:collapsed={node.collapsed === true}
					class:disabled={node.enabled === false}
					class:invalid={node.invalid}
					class:draft-target={connectionDraft?.snapNodeId === node.id}
					class="node"
					style:left={`${node.position.x}rem`}
					style:top={`${node.position.y}rem`}
					style:width={`${nodeWidth(node)}rem`}
					style:height={`${nodeHeight(node)}rem`}
					style:--node-accent={node.color ?? 'var(--ga-outline)'}
					style:--node-header-height={`${nodeHeaderHeight(node)}rem`}
					data-node-id={node.id}
					onpointerdown={(event) => selectNodeBody(event, node)}>
					{#if node.enabled === false && node.bypassConnections && node.bypassConnections.length > 0}
						<svg
							class="node-bypass-wires"
							viewBox={`0 0 ${nodeWidth(node)} ${nodeHeight(node)}`}
							aria-hidden="true">
							{#each node.bypassConnections as bypass (`${bypass.inputSocketId}:${bypass.outputSocketId}`)}
								{@const path = nodeBypassPath(node, bypass)}
								{#if path}
									<path
										d={path}
										style:--bypass-color={bypass.color ?? node.color ?? 'var(--ga-socket)'} />
								{/if}
							{/each}
						</svg>
					{/if}
					<div class="node-header">
						{#if node.collapsed === true && firstCollapsedSocket(node, 'input')}
							{@const socket = firstCollapsedSocket(node, 'input') as GraphSocket}
							<div class="header-socket-list inputs collapsed-sockets">
								<button
									type="button"
									disabled={socketDisabled(node, socket, 'input')}
									class:incompatible={socketDisabled(node, socket, 'input')}
									class:connected={collapsedSocketConnected(node, 'input')}
									class:draft-target={collapsedSocketDraftTarget(node, 'input')}
									class="socket header-socket input collapsed-socket"
									data-node-id={node.id}
									data-socket-id={socket.id}
									title={collapsedSocketTitle(node, 'input')}
									onpointerup={(event) => finishConnection(event, node.id, socket)}>
									<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
									></span>
									<span>{collapsedSocketLabel(node, 'input')}</span>
								</button>
							</div>
						{:else if node.socketPlacement === 'header'}
							<div class="header-socket-list inputs">
								{#each node.inputs as socket (socket.id)}
									<button
										type="button"
										disabled={socketDisabled(node, socket, 'input')}
										class:incompatible={socketDisabled(node, socket, 'input')}
										class:connected={connectedSockets.has(`${node.id}:${socket.id}`)}
										class:draft-target={connectionDraft?.snapNodeId === node.id &&
											connectionDraft?.snapSocketId === socket.id}
										class="socket header-socket input"
										data-node-id={node.id}
										data-socket-id={socket.id}
										title={socket.valueType ?? socket.label}
										onpointerup={(event) => finishConnection(event, node.id, socket)}>
										<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
										></span>
										<span>{socket.label}</span>
									</button>
								{/each}
							</div>
						{:else if node.headerInputs && node.headerInputs.length > 0}
							<div class="header-socket-list inputs header-inputs-extra">
								{#each node.headerInputs as socket (socket.id)}
									<button
										type="button"
										disabled={socketDisabled(node, socket, 'input')}
										class:incompatible={socketDisabled(node, socket, 'input')}
										class:connected={connectedSockets.has(`${node.id}:${socket.id}`)}
										class:draft-target={connectionDraft?.snapNodeId === node.id &&
											connectionDraft?.snapSocketId === socket.id}
										class="socket header-socket input"
										data-node-id={node.id}
										data-socket-id={socket.id}
										title={socket.valueType ?? socket.label}
										onpointerup={(event) => finishConnection(event, node.id, socket)}>
										<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
										></span>
									</button>
								{/each}
							</div>
						{/if}
						{#if renamingNodeId === node.id}
							<div
								class="node-title renaming"
								title={node.description?.trim() || node.subtitle || node.label}>
								<div class="node-title-row">
									{#if node.canDisable === true}
										<button
											type="button"
											class="node-enable"
											class:enabled={node.enabled !== false}
											role="switch"
											aria-checked={node.enabled !== false}
											aria-label={`${node.enabled === false ? 'Enable' : 'Disable'} ${node.label}`}
											title={node.enabled === false ? 'Enable node' : 'Disable node'}
											onpointerdown={(event) => event.stopPropagation()}
											onclick={(event) => toggleNodeEnabled(event, node)}></button>
									{/if}
									<input
										bind:this={renameInput}
										class="node-title-input"
										value={renameDraft}
										aria-label={`Rename ${node.label}`}
										oninput={(event) =>
											(renameDraft = (event.currentTarget as HTMLInputElement).value)}
										onkeydown={handleRenameKeydown}
										onblur={finishNodeRename}
										onpointerdown={(event) => event.stopPropagation()}
										onclick={(event) => event.stopPropagation()} />
								</div>
								{#if node.description?.trim()}
									<small>{node.description}</small>
								{:else if node.subtitle}
									<small>{node.subtitle}</small>
								{/if}
							</div>
						{:else}
							<!-- svelte-ignore a11y_no_static_element_interactions -->
							<div
								class="node-title"
								title={node.description?.trim() || node.subtitle || node.label}
								onpointerdown={(event) => handleNodeTitlePointerDown(event, node)}>
								<div class="node-title-row">
									{#if node.canDisable === true}
										<button
											type="button"
											class="node-enable"
											class:enabled={node.enabled !== false}
											role="switch"
											aria-checked={node.enabled !== false}
											aria-label={`${node.enabled === false ? 'Enable' : 'Disable'} ${node.label}`}
											title={node.enabled === false ? 'Enable node' : 'Disable node'}
											onpointerdown={(event) => event.stopPropagation()}
											onclick={(event) => toggleNodeEnabled(event, node)}></button>
									{/if}
									<button
										type="button"
										class="node-title-label"
										class:editable={canRenameNode(node)}
										aria-disabled={!canRenameNode(node)}
										title={node.description?.trim() || node.subtitle || node.label}
										ondblclick={(event) => beginNodeRename(event, node)}
										onpointerdown={(event) => handleNodeTitlePointerDown(event, node)}
										onclick={(event) => event.stopPropagation()}>
										<strong>{node.label}</strong>
									</button>
								</div>
								{#if node.description?.trim()}
									<small>{node.description}</small>
								{:else if node.subtitle}
									<small>{node.subtitle}</small>
								{/if}
							</div>
						{/if}
						{#if node.warning}
							<span class="node-warning" title={node.warning} aria-label={node.warning}>!</span>
						{/if}
						{#if nodeHeaderContent && node.collapsed !== true}
							<div class="node-header-content" data-no-node-select>
								{@render nodeHeaderContent(node)}
							</div>
						{/if}
						{#if node.collapsed === true && firstCollapsedSocket(node, 'output')}
							{@const socket = firstCollapsedSocket(node, 'output') as GraphSocket}
							<div class="header-socket-list outputs collapsed-sockets">
								<button
									type="button"
									class:incompatible={socket.compatible === false}
									class:connected={collapsedSocketConnected(node, 'output')}
									class="socket header-socket output collapsed-socket"
									title={collapsedSocketTitle(node, 'output')}
									onpointerdown={(event) => startConnection(event, node.id, socket)}>
									<span>{collapsedSocketLabel(node, 'output')}</span>
									<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
									></span>
								</button>
							</div>
						{:else if node.socketPlacement === 'header'}
							<div class="header-socket-list outputs">
								{#each node.outputs as socket (socket.id)}
									<button
										type="button"
										class:incompatible={socket.compatible === false}
										class:connected={connectedSockets.has(`${node.id}:${socket.id}`)}
										class="socket header-socket output"
										title={socket.valueType ?? socket.label}
										onpointerdown={(event) => startConnection(event, node.id, socket)}>
										<span>{socket.label}</span>
										<span class="pin" style:--socket-color={socket.color ?? 'var(--ga-socket)'}
										></span>
									</button>
								{/each}
							</div>
						{/if}
					</div>
					{#if node.collapsed !== true}
						<div class="node-body">
							{#if node.socketPlacement !== 'header'}
								<div class="socket-columns">
									<div
										class="socket-list inputs"
										style:--socket-default-width={`${socketDefaultColumnWidth(displaySockets(node, 'input', node.inputs))}rem`}>
										{#each displaySockets(node, 'input', node.inputs) as socket (socket.id)}
											<div class="socket-row input" class:component={socket.parentId}>
												<button
													type="button"
													disabled={socketDisabled(node, socket, 'input')}
													class:incompatible={socketDisabled(node, socket, 'input')}
													class:connected={hasSocketConnection(node, socket.id)}
													class:draft-target={connectionDraft?.snapNodeId === node.id &&
														connectionDraft?.snapSocketId === socket.id}
													class:component={socket.parentId}
													class="socket input"
													data-node-id={node.id}
													data-socket-id={socket.id}
													title={socket.valueType ?? socket.label}
													onpointerup={(event) => finishConnection(event, node.id, socket)}>
													<span
														class="pin"
														style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
													<span>{socket.label}</span>
												</button>
												{#if socket.children && socket.children.length > 0}
													<button
														type="button"
														class="socket-expander"
														aria-label={`${socketExpanded(node, socket, 'input') ? 'Collapse' : 'Expand'} ${socket.label}`}
														title={`${socketExpanded(node, socket, 'input') ? 'Collapse' : 'Expand'} ${socket.label}`}
														onclick={(event) => toggleSocketExpanded(event, node, socket, 'input')}>
														{socketExpanded(node, socket, 'input') ? 'v' : '>'}
													</button>
												{:else}
													<span class="socket-expander-spacer"></span>
												{/if}
												{#if inputSocketContent && inputSocketContentVisible(node, socket)}
													<div
														class="socket-inline-content input"
														data-no-node-select
														role="presentation"
														onpointerdown={(event) => event.stopPropagation()}>
														{@render inputSocketContent(node, socket)}
													</div>
												{/if}
											</div>
										{/each}
									</div>
									<div class="socket-list outputs">
										{#each displaySockets(node, 'output', node.outputs) as socket (socket.id)}
											<div class="socket-row output" class:component={socket.parentId}>
												{#if socket.children && socket.children.length > 0}
													<button
														type="button"
														class="socket-expander"
														aria-label={`${socketExpanded(node, socket, 'output') ? 'Collapse' : 'Expand'} ${socket.label}`}
														title={`${socketExpanded(node, socket, 'output') ? 'Collapse' : 'Expand'} ${socket.label}`}
														onclick={(event) =>
															toggleSocketExpanded(event, node, socket, 'output')}>
														{socketExpanded(node, socket, 'output') ? 'v' : '<'}
													</button>
												{:else}
													<span class="socket-expander-spacer"></span>
												{/if}
												{#if outputSocketContent}
													<div
														class="socket-inline-content output"
														data-no-node-select
														role="presentation"
														onpointerdown={(event) => event.stopPropagation()}>
														{@render outputSocketContent(node, socket)}
													</div>
												{/if}
												<button
													type="button"
													disabled={socketDisabled(node, socket, 'output')}
													class:incompatible={socketDisabled(node, socket, 'output')}
													class:connected={hasSocketConnection(node, socket.id)}
													class:component={socket.parentId}
													class="socket output"
													title={socket.valueType ?? socket.label}
													onpointerdown={(event) => startConnection(event, node.id, socket)}>
													<span>{socket.label}</span>
													<span
														class="pin"
														style:--socket-color={socket.color ?? 'var(--ga-socket)'}></span>
												</button>
											</div>
										{/each}
									</div>
								</div>
							{/if}
							{#if nodeContent}
								<div class="node-content" data-no-node-select>
									{@render nodeContent(node)}
								</div>
							{/if}
						</div>
					{/if}
					{#if node.resizable && node.collapsed !== true}
						<button
							type="button"
							class="resize-handle"
							aria-label={`Resize ${node.label}`}
							title={`Resize ${node.label}; double-click for automatic size`}
							onpointerdown={(event) => startNodeResize(event, node)}
							ondblclick={(event) => resetNodeSize(event, node)}></button>
					{/if}
				</article>
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

	.node {
		position: absolute;
		box-sizing: border-box;
		border: solid 0.08rem color-mix(in srgb, var(--node-accent) 44%, transparent);
		border-radius: 0.55rem;
		background: var(--ga-node);
		box-shadow: 0 0.55rem 1.3rem rgb(0 0 0 / 0.28);
		color: var(--gc-color-text, #e8edf6);
		filter: saturate(0.86);
		overflow: hidden;
		transition:
			width 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			height 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			border-color 0.46s ease,
			box-shadow 0.46s ease,
			filter 0.46s ease;
	}

	.graph-canvas.node-resizing .node {
		transition:
			border-color 0.46s ease,
			box-shadow 0.46s ease;
	}

	.node.selected {
		border-color: var(--ga-selection);
	}

	.node.active {
		border-color: color-mix(in srgb, var(--node-accent) 88%, white 12%);
		filter: saturate(1);
		box-shadow:
			inset 0 0 0 0.02rem color-mix(in srgb, var(--node-accent) 52%, transparent),
			0 0.55rem 1.3rem rgb(0 0 0 / 0.28),
			0 0 0.22rem color-mix(in srgb, var(--node-accent) 20%, transparent);
		transition:
			width 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			height 0.18s cubic-bezier(0.2, 0, 0.13, 1),
			border-color 0.06s ease-out,
			box-shadow 0.06s ease-out,
			filter 0.06s ease-out;
	}

	.node.disabled {
		border-style: dashed;
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 64%, transparent);
	}

	.node.selected.active {
		border-color: var(--ga-selection);
		box-shadow:
			inset 0 0 0 0.02rem color-mix(in srgb, var(--gc-color-selection) 58%, transparent),
			0 0.55rem 1.3rem rgb(0 0 0 / 0.28),
			0 0 0.5rem color-mix(in srgb, var(--gc-color-selection) 52%, transparent);
	}

	.node.invalid {
		border-color: var(--ga-error);
	}

	.node.draft-target {
		border-color: var(--ga-selection);
		box-shadow: 0 0 0.8rem color-mix(in srgb, var(--gc-color-selection) 60%, transparent);
	}

	.socket.draft-target .pin {
		background: var(--ga-selection);
		box-shadow: 0 0 0.4rem var(--ga-selection);
	}

	.node-bypass-wires {
		position: absolute;
		z-index: 1;
		inset: 0;
		inline-size: 100%;
		block-size: 100%;
		pointer-events: none;
	}

	.node-bypass-wires path {
		fill: none;
		stroke: var(--bypass-color, var(--ga-socket));
		stroke-width: 0.11rem;
		stroke-dasharray: 0.28rem 0.22rem;
		opacity: 0.78;
		vector-effect: non-scaling-stroke;
	}

	.node-header {
		position: relative;
		z-index: 2;
		display: flex;
		align-items: stretch;
		inline-size: 100%;
		block-size: var(--node-header-height, 1.8rem);
		min-block-size: var(--node-header-height, 1.8rem);
		box-sizing: border-box;
		border-block-end: solid 0.06rem color-mix(in srgb, var(--node-accent) 62%, transparent);
		border-radius: 0.48rem 0.48rem 0 0;
		background: color-mix(in srgb, var(--node-accent) 28%, var(--ga-node));
		color: inherit;
		overflow: hidden;
	}

	.node-body {
		position: relative;
		z-index: 2;
	}

	.node.collapsed .node-header {
		border-block-end: 0;
		border-radius: 0.48rem;
	}

	.node-enable {
		position: relative;
		flex: 0 0 0.85rem;
		align-self: center;
		inline-size: 0.85rem;
		block-size: 1rem;
		padding: 0;
		border: 0;
		background: transparent;
		cursor: pointer;
	}

	.node-enable::before {
		content: '';
		position: absolute;
		inset: 50% auto auto 50%;
		box-sizing: border-box;
		inline-size: 0.54rem;
		block-size: 0.54rem;
		border: solid 0.06rem color-mix(in srgb, var(--gc-color-text, #e8edf6) 56%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--ga-bg) 68%, transparent);
		transform: translate(-50%, -50%);
		transition:
			background 0.14s ease,
			border-color 0.14s ease,
			box-shadow 0.14s ease;
	}

	.node-enable.enabled::before {
		border-color: color-mix(in srgb, var(--node-accent) 78%, white 12%);
		background: color-mix(in srgb, var(--node-accent) 82%, white 6%);
		box-shadow: 0 0 0.28rem color-mix(in srgb, var(--node-accent) 70%, transparent);
	}

	.node-title {
		display: flex;
		flex: 1 1 auto;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		min-inline-size: 0;
		padding: 0.15rem 0.35rem;
		box-sizing: border-box;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		text-align: center;
		cursor: default;
	}

	.graph-canvas.node-dragging .node-title {
		cursor: grabbing;
	}

	.node-title.renaming {
		cursor: text;
	}

	.node-title-row {
		display: flex;
		align-items: center;
		justify-content: center;
		inline-size: 100%;
		min-inline-size: 0;
		gap: 0.18rem;
	}

	.node-title-label {
		display: block;
		flex: 0 1 auto;
		inline-size: auto;
		min-inline-size: 0;
		max-inline-size: 100%;
		padding: 0;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		font-weight: 700;
		text-align: center;
		cursor: default;
	}

	.node .node-title-label strong {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.node-title-input {
		flex: 1 1 auto;
		inline-size: 100%;
		min-inline-size: 0;
		padding: 0.12rem 0.2rem;
		border: solid 0.06rem color-mix(in srgb, var(--ga-selection) 76%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--ga-bg) 86%, transparent);
		color: inherit;
		font: inherit;
		font-size: 0.76rem;
		font-weight: 700;
		outline: none;
	}

	.node-warning {
		display: inline-grid;
		flex: 0 0 1.05rem;
		align-self: center;
		place-items: center;
		inline-size: 1.05rem;
		block-size: 1.05rem;
		margin-inline-end: 0.25rem;
		border: 0.06rem solid color-mix(in srgb, var(--ga-error) 74%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--ga-error) 18%, transparent);
		color: var(--ga-error);
		font-size: 0.72rem;
		font-weight: 800;
		line-height: 1;
	}

	/* .node.active .node-header {
		background: color-mix(in srgb, var(--ga-active) 22%, var(--ga-node));
	} */

	.node strong,
	.node small {
		display: block;
		overflow-wrap: anywhere;
		white-space: normal;
	}

	.node strong {
		font-size: 0.76rem;
		line-height: 1.08;
	}

	.node small {
		font-size: 0.64rem;
		line-height: 1.1;
		opacity: 0.62;
	}

	.header-socket-list {
		display: flex;
		flex: 0 0 auto;
		align-items: stretch;
		min-inline-size: 0;
	}

	.header-socket {
		block-size: 100%;
		min-block-size: 1.72rem;
		padding-inline: 0.4rem;
		font-size: 0.64rem;
	}

	.node-header-content {
		display: flex;
		flex: 0 1 auto;
		align-items: center;
		min-inline-size: 0;
		max-inline-size: 45%;
	}

	.collapsed-socket {
		min-inline-size: 1.75rem;
		justify-content: center;
	}

	.node-body {
		display: flex;
		flex-direction: column;
		box-sizing: border-box;
		block-size: calc(100% - var(--node-header-height, 1.8rem));
		min-block-size: 1.75rem;
	}

	.socket-columns {
		display: grid;
		grid-template-columns: minmax(0, 1fr) max-content;
		column-gap: 0.2rem;
		align-content: start;
		box-sizing: border-box;
		flex: 0 0 auto;
		min-block-size: 1.35rem;
		padding-block: 0.2rem;
	}

	.socket-list {
		display: flex;
		flex-direction: column;
		gap: 0;
		min-inline-size: 0;
	}

	.socket-list.inputs {
		display: grid;
		grid-template-columns: minmax(0, max-content) 0.85rem minmax(
				0,
				var(--socket-default-width, 0rem)
			);
		align-items: start;
		justify-self: start;
		max-inline-size: 100%;
		overflow: hidden;
	}

	.socket-list.outputs {
		min-inline-size: max-content;
	}

	.socket-row {
		display: flex;
		align-items: center;
		block-size: 1.35rem;
		min-inline-size: 0;
	}

	.socket-row.input {
		display: grid;
		grid-column: 1 / -1;
		grid-template-columns: subgrid;
		align-items: center;
		inline-size: 100%;
		max-inline-size: 100%;
	}

	.socket-row.output {
		justify-content: flex-end;
	}

	.socket-row.component {
		opacity: 0.9;
	}

	.socket-row.input.component {
		padding-inline-end: 0.45rem;
	}

	.socket-row.output.component {
		padding-inline-start: 0.45rem;
	}

	.socket-expander,
	.socket-expander-spacer {
		flex: 0 0 0.85rem;
		inline-size: 0.85rem;
		block-size: 100%;
	}

	.socket-expander {
		display: inline-grid;
		place-items: center;
		padding: 0;
		border: 0;
		background: transparent;
		color: color-mix(in srgb, var(--gc-color-text, #e8edf6) 58%, transparent);
		font: inherit;
		font-size: 0.58rem;
		cursor: pointer;
	}

	.socket-expander:hover {
		color: var(--gc-color-text, #e8edf6);
	}

	.socket-inline-content {
		display: flex;
		align-items: center;
		justify-content: stretch;
		min-inline-size: 0;
		max-inline-size: 100%;
		overflow: hidden;
	}

	.socket-inline-content.input {
		inline-size: 100%;
	}

	.socket-inline-content.output {
		flex: 0 1 auto;
		justify-content: flex-end;
		inline-size: auto;
		max-inline-size: 7rem;
	}

	.socket {
		position: relative;
		display: flex;
		align-items: center;
		gap: 0.25rem;
		block-size: 1.35rem;
		min-inline-size: 0;
		padding: 0 0.4rem;
		border: 0;
		background: transparent;
		color: inherit;
		font: inherit;
		font-size: 0.68rem;
		cursor: crosshair;
	}

	.socket-row .socket {
		flex: 0 0 auto;
		block-size: 100%;
	}

	/* In grid/subgrid context (input rows), override flex-dependent sizing */
	.socket-row.input .socket.input {
		block-size: 1.35rem;
		inline-size: 100%;
		max-inline-size: 100%;
	}

	.socket-columns .socket.input {
		justify-content: flex-start;
		/* padding-inline-start: 0.12rem; */
		padding-inline-end: 0.35rem;
	}

	.socket-columns .socket.output {
		justify-content: flex-end;
		padding-inline-start: 0.35rem;
		/* padding-inline-end: 0.12rem; */
	}

	.socket span:not(.pin) {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		transition: opacity 0.15s ease;
	}

	.graph-canvas[data-socket-labels='never'] .socket span:not(.pin) {
		opacity: 0;
	}

	.graph-canvas[data-socket-labels='hover'] .socket span:not(.pin) {
		opacity: 0;
	}

	.graph-canvas[data-socket-labels='hover'] .socket:hover span:not(.pin) {
		opacity: 1;
	}

	.socket.incompatible {
		opacity: 0.34;
		cursor: not-allowed;
	}

	.socket.component {
		font-size: 0.62rem;
	}

	.pin {
		position: relative;
		z-index: 0;
		box-sizing: border-box;
		inline-size: 0.5rem;
		block-size: 0.5rem;
		border: solid 0.05rem color-mix(in srgb, var(--socket-color) 70%, transparent);
		border-radius: 50%;
		background: var(--ga-node);
		box-shadow: 0 0 0 0.08rem rgb(0 0 0 / 0.32);
	}

	.socket:hover .pin,
	.socket.connected .pin {
		background: var(--socket-color);
	}

	.socket.connected .pin::after {
		content: '';
		position: absolute;
		top: 50%;
		transform: translateY(-50%);
		width: 0.75rem;
		height: 0.16rem;
		background: color-mix(in srgb, var(--socket-color) 72%, transparent);
		z-index: -1;
		pointer-events: none;
	}

	.socket.input.connected .pin::after {
		right: 50%;
	}

	.socket.output.connected .pin::after {
		left: 50%;
	}

	.node-content {
		flex: 1 1 auto;
		min-block-size: 0;
		/* padding: 0.18rem 0.35rem 0.35rem; */
		/* overflow: auto; */
		user-select: text;
	}

	.resize-handle {
		position: absolute;
		z-index: 3;
		inset-inline-end: 0.12rem;
		inset-block-end: 0.12rem;
		inline-size: 1rem;
		block-size: 1rem;
		padding: 0;
		border: 0;
		background: linear-gradient(
			135deg,
			transparent 0 52%,
			color-mix(in srgb, var(--ga-outline) 72%, transparent) 52% 60%,
			transparent 60% 68%,
			color-mix(in srgb, var(--ga-outline) 72%, transparent) 68% 76%,
			transparent 76%
		);
		cursor: nwse-resize;
	}

	.resize-handle:hover {
		background: linear-gradient(
			135deg,
			transparent 0 52%,
			color-mix(in srgb, var(--ga-selection) 88%, transparent) 52% 60%,
			transparent 60% 68%,
			color-mix(in srgb, var(--ga-selection) 88%, transparent) 68% 76%,
			transparent 76%
		);
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
