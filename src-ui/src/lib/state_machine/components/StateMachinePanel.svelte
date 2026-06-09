<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GraphCanvas,
		type GraphCamera,
		type GraphConnectionRequest,
		type GraphEdge,
		type GraphNode,
		type GraphNodeMove,
		type GraphNodePosition,
		type GraphNodeResize
	} from 'golden_alchemist_ui';
	import {
		ContextMenu,
		NodeAddButton,
		buildCreatableItemMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		type PanelProps,
		type PanelState,
		type ParamValue,
		type UiCreatableUserItem,
		type UiCreateUserItemInitialParam,
		type UiNodeDto
	} from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { createUiEditSession, sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import StateProcessorManager from './StateProcessorManager.svelte';

	const MANAGER_NODE_TYPE = 'state_machine_manager';
	const STATE_NODE_TYPE = 'state';
	const POSITION_X_DECL_ID = 'x';
	const POSITION_Y_DECL_ID = 'y';
	const WIDTH_DECL_ID = 'width';
	const HEIGHT_DECL_ID = 'height';
	const ACTIVE_DECL_ID = 'active';
	const PROCESSORS_DECL_ID = 'processors';
	const TRANSITIONS_DECL_ID = 'transitions';
	const TRANSITION_TARGET_DECL_ID = 'target';
	const TRANSITION_NODE_TYPE = 'state_transition';
	const TRANSITION_INPUT_SOCKET_ID = 'transition-input';
	const TRANSITION_OUTPUT_SOCKET_ID = 'transition-output';
	const DEFAULT_STATE_WIDTH_REM = 13;
	const DEFAULT_STATE_HEIGHT_REM = 8;
	const STATE_PLACEMENT_CLEARANCE_REM = 1;
	const STATE_PLACEMENT_STEP_REM = 2;
	const STATE_PLACEMENT_INDEX_CELL_REM = 8;
	const STATE_PLACEMENT_MAX_RING = 128;
	const CAMERA_PERSIST_DELAY_MS = 150;

	interface StateMachinePanelPersistedState {
		camera?: GraphCamera;
		avoidStateObstacles?: boolean;
	}

	const graphCamera = (value: unknown): GraphCamera | undefined => {
		if (typeof value !== 'object' || value === null) {
			return undefined;
		}
		const candidate = value as Record<string, unknown>;
		if (
			typeof candidate.x !== 'number' ||
			!Number.isFinite(candidate.x) ||
			typeof candidate.y !== 'number' ||
			!Number.isFinite(candidate.y) ||
			typeof candidate.zoom !== 'number' ||
			!Number.isFinite(candidate.zoom)
		) {
			return undefined;
		}
		return {
			x: candidate.x,
			y: candidate.y,
			zoom: candidate.zoom
		};
	};

	const camerasMatch = (left: GraphCamera | undefined, right: GraphCamera): boolean =>
		left !== undefined &&
		Math.abs(left.x - right.x) < 0.0001 &&
		Math.abs(left.y - right.y) < 0.0001 &&
		Math.abs(left.zoom - right.zoom) < 0.0001;

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	let panelRoot: HTMLElement | null = $state(null);
	let graphCanvas: {
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
		viewportCenter: () => GraphNodePosition;
	} | null = $state(null);
	let session = $derived(appState.session);
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuWorldPosition: GraphNodePosition | null = null;
	let geometryPersistenceTail = Promise.resolve();
	let pendingCamera: GraphCamera | null = null;
	let cameraPersistenceTimer: ReturnType<typeof setTimeout> | null = null;
	let avoidStateObstacles = $state(false);
	let initializedRoutingPanelId: string | null = null;

	$effect(() => {
		if (initializedRoutingPanelId === panelState.panelId) {
			return;
		}
		avoidStateObstacles =
			readPanelPersistedState<StateMachinePanelPersistedState>(panelState.params)
				.avoidStateObstacles === true;
		initializedRoutingPanelId = panelState.panelId;
	});

	let initialCamera = $derived.by(() =>
		graphCamera(readPanelPersistedState<StateMachinePanelPersistedState>(panelState.params).camera)
	);

	let manager = $derived.by(() => {
		if (!session) {
			return null;
		}
		const graph = session.graph.state;
		const root = graph.rootId === null ? null : graph.nodesById.get(graph.rootId);
		if (!root) {
			return null;
		}
		return (
			root.children
				.map((nodeId) => graph.nodesById.get(nodeId))
				.find((node) => node?.node_type === MANAGER_NODE_TYPE) ?? null
		);
	});

	let stateNodes = $derived.by(() => {
		if (!session || !manager) {
			return [];
		}
		return manager.children
			.map((nodeId) => session?.graph.state.nodesById.get(nodeId))
			.filter((node): node is UiNodeDto => node?.node_type === STATE_NODE_TYPE);
	});

	const parameterChild = (node: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!session) {
			return null;
		}
		for (const childId of node.children) {
			const child = session.graph.state.nodesById.get(childId);
			if (child?.decl_id === declId && child.data.kind === 'parameter') {
				return child;
			}
		}
		return null;
	};

	const declaredChild = (node: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!session) {
			return null;
		}
		for (const childId of node.children) {
			const child = session.graph.state.nodesById.get(childId);
			if (child?.decl_id === declId) {
				return child;
			}
		}
		return null;
	};

	const floatParameterValue = (node: UiNodeDto, declId: string): number | null => {
		const parameter = parameterChild(node, declId);
		if (parameter?.data.kind !== 'parameter' || parameter.data.param.value.kind !== 'float') {
			return null;
		}
		return parameter.data.param.value.value;
	};

	const boolParameterValue = (node: UiNodeDto, declId: string): boolean | null => {
		const parameter = parameterChild(node, declId);
		if (parameter?.data.kind !== 'parameter' || parameter.data.param.value.kind !== 'bool') {
			return null;
		}
		return parameter.data.param.value.value;
	};

	const graphPosition = (node: UiNodeDto, index: number): GraphNodePosition => {
		const x = floatParameterValue(node, POSITION_X_DECL_ID);
		const y = floatParameterValue(node, POSITION_Y_DECL_ID);
		if (x === null || y === null) {
			return {
				x: 2 + (index % 4) * 16,
				y: 2 + Math.floor(index / 4) * 7
			};
		}
		return { x, y };
	};

	const graphDimension = (node: UiNodeDto, declId: string, fallback: number): number => {
		const value = floatParameterValue(node, declId);
		return value !== null && value > 0 ? value : fallback;
	};

	let graphNodes = $derived.by((): GraphNode[] =>
		stateNodes.map((node, index) => ({
			id: String(node.node_id),
			label: node.meta.label,
			...graphPosition(node, index),
			width: graphDimension(node, WIDTH_DECL_ID, DEFAULT_STATE_WIDTH_REM),
			height: graphDimension(node, HEIGHT_DECL_ID, DEFAULT_STATE_HEIGHT_REM),
			resizable: true,
			socketPlacement: 'header',
			active: boolParameterValue(node, ACTIVE_DECL_ID) ?? false,
			inputs: [
				{
					id: TRANSITION_INPUT_SOCKET_ID,
					label: 'In',
					valueType: 'State transition'
				}
			],
			outputs: [
				{
					id: TRANSITION_OUTPUT_SOCKET_ID,
					label: 'Out',
					valueType: 'State transition'
				}
			]
		}))
	);
	let stateNodeIds = $derived(new Set(stateNodes.map((node) => node.node_id)));
	let selectedStateNodeIds = $derived(
		new Set((session?.selectedNodesIds ?? []).filter((nodeId) => stateNodeIds.has(nodeId)))
	);
	let statesByUuid = $derived(new Map(stateNodes.map((node) => [node.uuid, node])));
	let graphEdges = $derived.by((): GraphEdge[] => {
		if (!session) {
			return [];
		}
		return stateNodes.flatMap((sourceState) => {
			const transitionManager = declaredChild(sourceState, TRANSITIONS_DECL_ID);
			if (!transitionManager) {
				return [];
			}
			return transitionManager.children.flatMap((transitionId) => {
				const transition = session?.graph.state.nodesById.get(transitionId);
				if (!transition || transition.node_type !== TRANSITION_NODE_TYPE) {
					return [];
				}
				const targetParam = parameterChild(transition, TRANSITION_TARGET_DECL_ID);
				if (
					targetParam?.data.kind !== 'parameter' ||
					targetParam.data.param.value.kind !== 'reference'
				) {
					return [];
				}
				const reference = targetParam.data.param.value;
				const cachedTarget =
					reference.cached_id === undefined
						? undefined
						: session.graph.state.nodesById.get(reference.cached_id);
				const targetState =
					(cachedTarget?.uuid === reference.uuid
						? cachedTarget
						: statesByUuid.get(reference.uuid)) ?? null;
				if (!targetState || targetState.node_type !== STATE_NODE_TYPE) {
					return [];
				}
				const sourceSelected = selectedStateNodeIds.has(sourceState.node_id);
				const targetSelected = selectedStateNodeIds.has(targetState.node_id);
				return [
					{
						id: String(transition.node_id),
						from: {
							nodeId: String(sourceState.node_id),
							socketId: TRANSITION_OUTPUT_SOCKET_ID
						},
						to: {
							nodeId: String(targetState.node_id),
							socketId: TRANSITION_INPUT_SOCKET_ID
						},
						color: sourceSelected
							? 'var(--gc-color-selection, #ff5ba7)'
							: targetSelected
								? '#58a6ff'
								: undefined,
						active: boolParameterValue(sourceState, ACTIVE_DECL_ID) ?? false
					}
				];
			});
		});
	});
	let transitionNodeIds = $derived(
		new Set(
			graphEdges.flatMap((edge) => {
				const nodeId = Number(edge.id);
				return Number.isSafeInteger(nodeId) ? [nodeId] : [];
			})
		)
	);
	let selectedNodeIds = $derived(
		(session?.selectedNodesIds ?? [])
			.filter((nodeId) => stateNodeIds.has(nodeId))
			.map((nodeId) => String(nodeId))
	);
	let selectedEdgeIds = $derived(
		(session?.selectedNodesIds ?? [])
			.filter((nodeId) => transitionNodeIds.has(nodeId))
			.map((nodeId) => String(nodeId))
	);
	let emptyLabel = $derived(
		manager
			? 'Create a State from the + menu on the State Machine manager.'
			: 'The State Machine manager is not available in this project.'
	);
	let contextMenuAnchor = $derived.by(
		(): ContextMenuAnchor => ({
			kind: 'point',
			x: contextMenuX,
			y: contextMenuY
		})
	);

	const panelOwnsFocus = (): boolean =>
		panelRoot !== null &&
		document.activeElement !== null &&
		panelRoot.contains(document.activeElement);

	const selectNodes = (nodeIds: string[]): void => {
		if (!session) {
			return;
		}
		const hierarchyNodeIds = nodeIds
			.map((nodeId) => Number(nodeId))
			.filter((nodeId) => Number.isSafeInteger(nodeId) && stateNodeIds.has(nodeId));
		if (hierarchyNodeIds.length === 0) {
			session.clearSelection();
			return;
		}
		session.selectNodes(hierarchyNodeIds, 'REPLACE');
	};

	const selectEdges = (edgeIds: string[]): void => {
		if (!session) {
			return;
		}
		const hierarchyNodeIds = edgeIds
			.map((edgeId) => Number(edgeId))
			.filter((nodeId) => Number.isSafeInteger(nodeId) && transitionNodeIds.has(nodeId));
		if (hierarchyNodeIds.length === 0) {
			session.clearSelection();
			return;
		}
		session.selectNodes(hierarchyNodeIds, 'REPLACE');
	};

	const setAvoidStateObstacles = (event: Event): void => {
		avoidStateObstacles = (event.currentTarget as HTMLInputElement).checked;
		writePanelPersistedState(props.panelApi, { avoidStateObstacles });
	};

	const rectanglesOverlap = (
		left: GraphNodePosition,
		right: GraphNodePosition,
		rightWidth: number,
		rightHeight: number
	): boolean =>
		left.x < right.x + rightWidth + STATE_PLACEMENT_CLEARANCE_REM &&
		left.x + DEFAULT_STATE_WIDTH_REM + STATE_PLACEMENT_CLEARANCE_REM > right.x &&
		left.y < right.y + rightHeight + STATE_PLACEMENT_CLEARANCE_REM &&
		left.y + DEFAULT_STATE_HEIGHT_REM + STATE_PLACEMENT_CLEARANCE_REM > right.y;

	const placementCellKey = (x: number, y: number): string => `${x}:${y}`;

	const statePlacementIndex = (): Map<string, GraphNode[]> => {
		const index = new Map<string, GraphNode[]>();
		for (const node of graphNodes) {
			const width = node.width ?? DEFAULT_STATE_WIDTH_REM;
			const height = node.height ?? DEFAULT_STATE_HEIGHT_REM;
			const firstX = Math.floor(
				(node.x - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const lastX = Math.floor(
				(node.x + width + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const firstY = Math.floor(
				(node.y - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const lastY = Math.floor(
				(node.y + height + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			for (let cellX = firstX; cellX <= lastX; cellX += 1) {
				for (let cellY = firstY; cellY <= lastY; cellY += 1) {
					const key = placementCellKey(cellX, cellY);
					const occupants = index.get(key);
					if (occupants) {
						occupants.push(node);
					} else {
						index.set(key, [node]);
					}
				}
			}
		}
		return index;
	};

	const placementIsFree = (
		candidate: GraphNodePosition,
		index: Map<string, GraphNode[]>
	): boolean => {
		const firstX = Math.floor(
			(candidate.x - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		const lastX = Math.floor(
			(candidate.x + DEFAULT_STATE_WIDTH_REM + STATE_PLACEMENT_CLEARANCE_REM) /
				STATE_PLACEMENT_INDEX_CELL_REM
		);
		const firstY = Math.floor(
			(candidate.y - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
		);
		const lastY = Math.floor(
			(candidate.y + DEFAULT_STATE_HEIGHT_REM + STATE_PLACEMENT_CLEARANCE_REM) /
				STATE_PLACEMENT_INDEX_CELL_REM
		);
		const nearbyNodes = new Set<GraphNode>();
		for (let cellX = firstX; cellX <= lastX; cellX += 1) {
			for (let cellY = firstY; cellY <= lastY; cellY += 1) {
				for (const node of index.get(placementCellKey(cellX, cellY)) ?? []) {
					nearbyNodes.add(node);
				}
			}
		}
		for (const node of nearbyNodes) {
			if (
				rectanglesOverlap(
					candidate,
					node,
					node.width ?? DEFAULT_STATE_WIDTH_REM,
					node.height ?? DEFAULT_STATE_HEIGHT_REM
				)
			) {
				return false;
			}
		}
		return true;
	};

	const nearestFreeStatePosition = (center: GraphNodePosition): GraphNodePosition => {
		const index = statePlacementIndex();
		const origin = {
			x: Math.round((center.x - DEFAULT_STATE_WIDTH_REM * 0.5) * 2) / 2,
			y: Math.round((center.y - DEFAULT_STATE_HEIGHT_REM * 0.5) * 2) / 2
		};
		if (placementIsFree(origin, index)) {
			return origin;
		}
		for (let ring = 1; ring <= STATE_PLACEMENT_MAX_RING; ring += 1) {
			const offsets: GraphNodePosition[] = [];
			for (let axis = -ring; axis <= ring; axis += 1) {
				offsets.push(
					{ x: axis, y: -ring },
					{ x: axis, y: ring },
					{ x: -ring, y: axis },
					{ x: ring, y: axis }
				);
			}
			offsets.sort((left, right) => left.x ** 2 + left.y ** 2 - (right.x ** 2 + right.y ** 2));
			for (const offset of offsets) {
				const candidate = {
					x: origin.x + offset.x * STATE_PLACEMENT_STEP_REM,
					y: origin.y + offset.y * STATE_PLACEMENT_STEP_REM
				};
				if (placementIsFree(candidate, index)) {
					return candidate;
				}
			}
		}
		return origin;
	};

	const initialParam = (decl_id: string, value: ParamValue): UiCreateUserItemInitialParam => ({
		decl_id,
		value
	});

	const createState = async (
		item: UiCreatableUserItem,
		preferredCenter?: GraphNodePosition | null
	): Promise<void> => {
		if (!manager) {
			return;
		}
		contextMenuOpen = false;
		const center = preferredCenter ?? graphCanvas?.viewportCenter() ?? { x: 0, y: 0 };
		const position = nearestFreeStatePosition(center);
		const result = await sendCreateUserItemByTypeIntent(
			manager.node_id,
			item.node_type,
			item.label,
			{
				select_when_created: item.select_when_created,
				initial_params: [
					initialParam(POSITION_X_DECL_ID, { kind: 'float', value: position.x }),
					initialParam(POSITION_Y_DECL_ID, { kind: 'float', value: position.y })
				]
			}
		);
		if (result.selectWhenCreated && result.createdNodeId !== null) {
			session?.selectNode(result.createdNodeId, 'REPLACE');
		}
	};

	let contextMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!manager || manager.creatable_user_items.length === 0) {
			return [];
		}
		return [
			{
				id: 'add-state-machine-item',
				label: 'Add...',
				icon: addIcon,
				submenu: buildCreatableItemMenu(manager.creatable_user_items, (item) =>
					createState(item, contextMenuWorldPosition)
				)
			}
		];
	});

	const openContextMenu = (event: MouseEvent, position: GraphNodePosition): void => {
		if (contextMenuItems.length === 0) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		contextMenuX = event.clientX;
		contextMenuY = event.clientY;
		contextMenuWorldPosition = position;
		contextMenuOpen = true;
		graphCanvas?.focus();
	};

	const connectStates = async (connection: GraphConnectionRequest): Promise<void> => {
		if (
			connection.from.socketId !== TRANSITION_OUTPUT_SOCKET_ID ||
			connection.to.socketId !== TRANSITION_INPUT_SOCKET_ID ||
			!session
		) {
			return;
		}
		const sourceState = session.graph.state.nodesById.get(Number(connection.from.nodeId));
		const targetState = session.graph.state.nodesById.get(Number(connection.to.nodeId));
		if (
			!sourceState ||
			sourceState.node_type !== STATE_NODE_TYPE ||
			!targetState ||
			targetState.node_type !== STATE_NODE_TYPE
		) {
			return;
		}
		const transitionManager = declaredChild(sourceState, TRANSITIONS_DECL_ID);
		const transitionItem = transitionManager?.creatable_user_items.find(
			(item) => item.node_type === TRANSITION_NODE_TYPE
		);
		if (!transitionManager || !transitionItem) {
			return;
		}
		await sendCreateUserItemByTypeIntent(
			transitionManager.node_id,
			transitionItem.node_type,
			transitionItem.label,
			{
				select_when_created: false,
				initial_params: [
					initialParam(TRANSITION_TARGET_DECL_ID, {
						kind: 'reference',
						uuid: targetState.uuid,
						cached_id: targetState.node_id,
						cached_name: targetState.meta.label,
						relative_path_from_root: []
					})
				]
			}
		);
	};

	const processorManagerForGraphNode = (graphNode: GraphNode): UiNodeDto | null => {
		const stateNode = session?.graph.state.nodesById.get(Number(graphNode.id));
		return stateNode ? declaredChild(stateNode, PROCESSORS_DECL_ID) : null;
	};

	const persistNodePositionsNow = async (moves: GraphNodeMove[]): Promise<void> => {
		if (!session || moves.length === 0) {
			return;
		}

		const writableMoves = moves.flatMap((move) => {
			const stateNode = session.graph.state.nodesById.get(Number(move.nodeId));
			if (!stateNode || stateNode.node_type !== STATE_NODE_TYPE) {
				return [];
			}
			const xParameter = parameterChild(stateNode, POSITION_X_DECL_ID);
			const yParameter = parameterChild(stateNode, POSITION_Y_DECL_ID);
			if (
				xParameter?.data.kind !== 'parameter' ||
				yParameter?.data.kind !== 'parameter' ||
				xParameter.data.param.read_only ||
				yParameter.data.param.read_only
			) {
				return [];
			}
			return [
				{
					move,
					stateNode,
					xParameterId: xParameter.node_id,
					xBehaviour: xParameter.data.param.event_behaviour,
					yParameterId: yParameter.node_id,
					yBehaviour: yParameter.data.param.event_behaviour
				}
			];
		});
		if (writableMoves.length !== moves.length) {
			throw new Error('one or more selected states cannot persist canvas positions');
		}

		const editSession = createUiEditSession(
			writableMoves.length === 1
				? `Move ${writableMoves[0].stateNode.meta.label}`
				: `Move ${writableMoves.length} states`,
			'state-position'
		);
		await editSession.begin();
		if (!editSession.active) {
			throw new Error('another edit session is already active');
		}
		try {
			await Promise.all(
				writableMoves.flatMap(({ move, xParameterId, xBehaviour, yParameterId, yBehaviour }) => [
					session.sendIntent({
						kind: 'setParam',
						node: xParameterId,
						value: { kind: 'float', value: move.position.x },
						behaviour: xBehaviour
					}),
					session.sendIntent({
						kind: 'setParam',
						node: yParameterId,
						value: { kind: 'float', value: move.position.y },
						behaviour: yBehaviour
					})
				])
			);
		} finally {
			await editSession.end();
		}
	};

	const persistNodePositions = (moves: GraphNodeMove[]): Promise<void> => {
		const operation = geometryPersistenceTail
			.catch(() => undefined)
			.then(() => persistNodePositionsNow(moves));
		geometryPersistenceTail = operation.catch(() => undefined);
		return operation;
	};

	const persistNodeResizeNow = async (resize: GraphNodeResize): Promise<void> => {
		if (!session) {
			return;
		}
		const stateNode = session.graph.state.nodesById.get(Number(resize.nodeId));
		if (!stateNode || stateNode.node_type !== STATE_NODE_TYPE) {
			throw new Error('resized node is not a State');
		}
		const widthParameter = parameterChild(stateNode, WIDTH_DECL_ID);
		const heightParameter = parameterChild(stateNode, HEIGHT_DECL_ID);
		if (
			widthParameter?.data.kind !== 'parameter' ||
			heightParameter?.data.kind !== 'parameter' ||
			widthParameter.data.param.read_only ||
			heightParameter.data.param.read_only
		) {
			throw new Error('State cannot persist canvas dimensions');
		}

		const editSession = createUiEditSession(`Resize ${stateNode.meta.label}`, 'state-size');
		await editSession.begin();
		if (!editSession.active) {
			throw new Error('another edit session is already active');
		}
		try {
			await Promise.all([
				session.sendIntent({
					kind: 'setParam',
					node: widthParameter.node_id,
					value: { kind: 'float', value: resize.size.width },
					behaviour: widthParameter.data.param.event_behaviour
				}),
				session.sendIntent({
					kind: 'setParam',
					node: heightParameter.node_id,
					value: { kind: 'float', value: resize.size.height },
					behaviour: heightParameter.data.param.event_behaviour
				})
			]);
		} finally {
			await editSession.end();
		}
	};

	const persistNodeResize = (resize: GraphNodeResize): Promise<void> => {
		const operation = geometryPersistenceTail
			.catch(() => undefined)
			.then(() => persistNodeResizeNow(resize));
		geometryPersistenceTail = operation.catch(() => undefined);
		return operation;
	};

	const flushCameraPersistence = (): void => {
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
			cameraPersistenceTimer = null;
		}
		const nextCamera = pendingCamera;
		pendingCamera = null;
		if (!nextCamera) {
			return;
		}
		const currentCamera = graphCamera(
			readPanelPersistedState<StateMachinePanelPersistedState>(props.panelApi.getParams()).camera
		);
		if (camerasMatch(currentCamera, nextCamera)) {
			return;
		}
		writePanelPersistedState(props.panelApi, { camera: nextCamera });
	};

	const persistCamera = (camera: GraphCamera): void => {
		pendingCamera = { ...camera };
		if (cameraPersistenceTimer !== null) {
			clearTimeout(cameraPersistenceTimer);
		}
		cameraPersistenceTimer = setTimeout(flushCameraPersistence, CAMERA_PERSIST_DELAY_MS);
	};

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	onMount(() => {
		const unregisterFrame = registerCommandHandler(
			'view.frame',
			() => (panelOwnsFocus() ? (graphCanvas?.frameSelection() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterHome = registerCommandHandler(
			'view.home',
			() => (panelOwnsFocus() ? (graphCanvas?.home() ?? false) : false),
			{ priority: 100 }
		);
		return () => {
			unregisterFrame();
			unregisterHome();
			flushCameraPersistence();
		};
	});
</script>

{#snippet stateNodeContent(graphNode: GraphNode)}
	{@const processorManager = processorManagerForGraphNode(graphNode)}
	{#if processorManager}
		<StateProcessorManager manager={processorManager} />
	{/if}
{/snippet}

<section
	bind:this={panelRoot}
	class="state-machine-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphCanvas?.focus()}>
	{#if manager}
		<div class="state-machine-actions">
			<NodeAddButton
				node={manager}
				onCreateItem={(item) => createState(item, graphCanvas?.viewportCenter())} />
			<label class="wire-routing-toggle" title="Automatically route transition wires around States">
				<input type="checkbox" checked={avoidStateObstacles} onchange={setAvoidStateObstacles} />
				<span>Avoid states</span>
			</label>
		</div>
	{/if}

	<GraphCanvas
		bind:this={graphCanvas}
		nodes={graphNodes}
		edges={graphEdges}
		{selectedNodeIds}
		{selectedEdgeIds}
		onSelectionChange={selectNodes}
		onEdgeSelectionChange={selectEdges}
		onNodesMove={persistNodePositions}
		onNodeResize={persistNodeResize}
		onConnect={connectStates}
		nodeContent={stateNodeContent}
		onBackgroundContextMenu={openContextMenu}
		routeEdgesAroundNodes={avoidStateObstacles}
		{initialCamera}
		onCameraChange={persistCamera}
		{emptyLabel} />

	<ContextMenu
		bind:open={contextMenuOpen}
		items={contextMenuItems}
		anchor={contextMenuAnchor}
		minWidthRem={10}
		maxWidthCss="max-content" />
</section>

<style>
	.state-machine-panel {
		position: relative;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
		border-radius: 0.5rem;
	}

	.state-machine-actions {
		position: absolute;
		inset-block-start: 0.7rem;
		inset-inline-start: 0.7rem;
		z-index: 20;
		display: flex;
		align-items: center;
		gap: 0.3rem;
		padding: 0.15rem;
	}

	.wire-routing-toggle {
		display: inline-flex;
		align-items: center;
		gap: 0.28rem;
		min-block-size: 1.45rem;
		padding-inline: 0.4rem;
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-background) 84%, transparent);
		color: color-mix(in srgb, var(--gc-color-text) 78%, transparent);
		font-size: 0.66rem;
		cursor: pointer;
		backdrop-filter: blur(0.5rem);
	}

	.wire-routing-toggle input {
		inline-size: 0.8rem;
		block-size: 0.8rem;
		margin: 0;
	}
</style>
