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
		type GraphNodeResize,
		type GraphNodeSize
	} from 'golden_graph_ui';
	import {
		STATECHART_INCOMING_PORT_ID,
		STATECHART_OUTGOING_PORT_ID,
		StatechartDocumentView
	} from '../document';
	import {
		ContextMenu,
		buildCreatableItemMenu,
		type ContextMenuAnchor,
		type ContextMenuItem,
		type PanelProps,
		type PanelState,
		type ParamValue,
		type UiColorDto,
		type UiCreatableUserItem,
		type UiCreateUserItemInitialParam,
		type UiNodeDto
	} from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { openNodeContextMenu } from 'golden_ui/store/node-context-menu.svelte';
	import {
		createUiEditSession,
		sendCreateUserItemByTypeIntent,
		sendPatchMetaIntent
	} from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';
	import GraphToolbarActions from '../../alchemist/components/GraphToolbarActions.svelte';
	import StateProcessorManager from '../../alchemist/components/StateProcessorManager.svelte';
	import type { ProcessorOverviewDemandDto } from '../generated';
	import { STATE_MACHINE_PROCESSOR_OVERVIEW_DEMAND_TOPIC } from '../processorOverview.svelte';

	const MANAGER_NODE_TYPE = 'state_machine_manager';
	const STATE_NODE_TYPE = 'state';
	const POSITION_DECL_ID = 'position';
	const SIZE_DECL_ID = 'size';
	const PROCESSORS_DECL_ID = 'processors';
	const TRANSITIONS_DECL_ID = 'transitions';
	const TRANSITION_TARGET_DECL_ID = 'target';
	const TRANSITION_NODE_TYPE = 'state_transition';
	const TRANSITION_INPUT_SOCKET_ID = STATECHART_INCOMING_PORT_ID;
	const TRANSITION_OUTPUT_SOCKET_ID = STATECHART_OUTGOING_PORT_ID;
	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PROCESSOR_FOLDER_NODE_TYPE = 'state_processor_folder';
	const DEFAULT_STATE_WIDTH_REM = 13;
	const DEFAULT_STATE_HEIGHT_REM = 8;
	const STATE_AUTO_HEADER_REM = 1.9;
	const STATE_AUTO_PROCESSOR_HEADER_REM = 1.45;
	const STATE_AUTO_LIST_PADDING_REM = 0.4;
	const STATE_AUTO_PROCESSOR_ROW_REM = 1.45;
	const STATE_AUTO_RESIZER_MARGIN_REM = 1.2;
	const STATE_PLACEMENT_CLEARANCE_REM = 1;
	const STATE_PLACEMENT_STEP_REM = 2;
	const STATE_PLACEMENT_INDEX_CELL_REM = 8;
	const STATE_PLACEMENT_MAX_RING = 128;
	const CAMERA_PERSIST_DELAY_MS = 150;
	const PROCESSOR_OVERVIEW_VISIBILITY_SETTLE_MS = 200;
	const PROCESSOR_OVERVIEW_HEARTBEAT_MS = 2000;
	const processorOverviewSubscriptionId = `state-machine-overview:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`;

	interface StateMachinePanelPersistedState {
		camera?: GraphCamera;
		autoWire?: boolean;
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
	let processorVisibilitySettleTimer: ReturnType<typeof setTimeout> | null = null;
	let pendingVisibleStateIds: string[] = [];
	let settledVisibleStateIds = $state<string[]>([]);
	let autoWire = $state(false);
	let initializedAutoWirePanelId: string | null = null;

	$effect(() => {
		if (initializedAutoWirePanelId === panelState.panelId) {
			return;
		}
		autoWire =
			readPanelPersistedState<StateMachinePanelPersistedState>(panelState.params).autoWire === true;
		initializedAutoWirePanelId = panelState.panelId;
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

	const publishProcessorOverviewDemand = (processorIds: string[]): void => {
		const activeSession = session;
		const stateMachineManager = manager;
		if (!activeSession || activeSession.status !== 'connected' || !stateMachineManager) return;
		const payload: ProcessorOverviewDemandDto = {
			subscription_id: processorOverviewSubscriptionId,
			processor_ids: processorIds
		};
		void activeSession
			.sendIntent({
				kind: 'sendNodeEvent',
				node: stateMachineManager.node_id,
				topic: STATE_MACHINE_PROCESSOR_OVERVIEW_DEMAND_TOPIC,
				payload
			})
			.catch(() => undefined);
	};

	const stringArraysMatch = (left: string[], right: string[]): boolean =>
		left.length === right.length && left.every((value, index) => value === right[index]);

	const settleVisibleStateIds = (nodeIds: string[]): void => {
		pendingVisibleStateIds = [...nodeIds].sort();
		if (processorVisibilitySettleTimer !== null) {
			clearTimeout(processorVisibilitySettleTimer);
		}
		processorVisibilitySettleTimer = setTimeout(() => {
			processorVisibilitySettleTimer = null;
			if (!stringArraysMatch(settledVisibleStateIds, pendingVisibleStateIds)) {
				settledVisibleStateIds = pendingVisibleStateIds;
			}
		}, PROCESSOR_OVERVIEW_VISIBILITY_SETTLE_MS);
	};

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

	const vec2ParameterValue = (node: UiNodeDto, declId: string): [number, number] | null => {
		const parameter = parameterChild(node, declId);
		if (parameter?.data.kind !== 'parameter' || parameter.data.param.value.kind !== 'vec2') {
			return null;
		}
		return parameter.data.param.value.value;
	};

	const stringParameterValue = (node: UiNodeDto, declId: string): string | null => {
		const parameter = parameterChild(node, declId);
		if (parameter?.data.kind !== 'parameter' || parameter.data.param.value.kind !== 'str') {
			return null;
		}
		return parameter.data.param.value.value;
	};

	const graphPosition = (node: UiNodeDto, index: number): GraphNodePosition => {
		const position = vec2ParameterValue(node, POSITION_DECL_ID);
		if (position === null) {
			return {
				x: 2 + (index % 4) * 16,
				y: 2 + Math.floor(index / 4) * 7
			};
		}
		return { x: position[0], y: position[1] };
	};

	const graphSize = (node: UiNodeDto): { width: number; height: number } | undefined => {
		const parameter = parameterChild(node, SIZE_DECL_ID);
		if (
			parameter?.data.kind !== 'parameter' ||
			!parameter.meta.enabled ||
			parameter.data.param.value.kind !== 'vec2'
		) {
			return undefined;
		}
		const [width, height] = parameter.data.param.value.value;
		return width > 0 && height > 0 ? { width, height } : undefined;
	};

	const isProcessorTreeNode = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		Boolean(
			node &&
			(node.user_item_kind === PROCESSOR_ITEM_KIND || node.node_type === PROCESSOR_FOLDER_NODE_TYPE)
		);

	const processorTreeRowCount = (node: UiNodeDto): number => {
		if (
			node.node_type !== PROCESSOR_FOLDER_NODE_TYPE ||
			node.meta.presentation?.collapsed === true
		) {
			return 1;
		}
		let count = 1;
		for (const childId of node.children) {
			const child = session?.graph.state.nodesById.get(childId);
			if (isProcessorTreeNode(child)) {
				count += processorTreeRowCount(child);
			}
		}
		return count;
	};

	const processorManagerRowCount = (manager: UiNodeDto | null): number => {
		if (!manager) {
			return 1;
		}
		let count = 0;
		for (const childId of manager.children) {
			const child = session?.graph.state.nodesById.get(childId);
			if (isProcessorTreeNode(child)) {
				count += processorTreeRowCount(child);
			}
		}
		return Math.max(1, count);
	};

	const graphAutomaticSize = (node: UiNodeDto): GraphNodeSize => {
		const rows = processorManagerRowCount(declaredChild(node, PROCESSORS_DECL_ID));
		const contentHeight =
			STATE_AUTO_HEADER_REM +
			STATE_AUTO_PROCESSOR_HEADER_REM +
			STATE_AUTO_LIST_PADDING_REM +
			rows * STATE_AUTO_PROCESSOR_ROW_REM +
			STATE_AUTO_RESIZER_MARGIN_REM;
		return {
			width: DEFAULT_STATE_WIDTH_REM,
			height: Math.max(DEFAULT_STATE_HEIGHT_REM, contentHeight)
		};
	};

	const graphNodeWidth = (node: GraphNode): number =>
		node.size?.width ?? node.automaticSize?.width ?? DEFAULT_STATE_WIDTH_REM;

	const graphNodeHeight = (node: GraphNode): number =>
		node.size?.height ?? node.automaticSize?.height ?? DEFAULT_STATE_HEIGHT_REM;

	const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

	const metadataColor = (color: UiColorDto | null | undefined): string | undefined => {
		if (!color) {
			return undefined;
		}
		const red = Math.round(clamp01(color.r) * 255);
		const green = Math.round(clamp01(color.g) * 255);
		const blue = Math.round(clamp01(color.b) * 255);
		return `rgb(${red} ${green} ${blue} / ${clamp01(color.a)})`;
	};

	const presentationColor = (
		presentation: UiNodeDto['meta']['presentation'] | null | undefined
	): string | undefined => metadataColor(presentation?.color ?? presentation?.default_color);

	let graphNodes = $derived.by((): GraphNode[] =>
		stateNodes.map((node, index) => ({
			id: String(node.node_id),
			label: node.meta.label,
			description: stringParameterValue(node, 'description')?.trim() || undefined,
			canRename: node.meta.user_permissions?.can_edit_name !== false,
			collapsed: node.meta.presentation?.collapsed === true,
			color: presentationColor(node.meta.presentation),
			position: graphPosition(node, index),
			size: graphSize(node),
			automaticSize: graphAutomaticSize(node),
			resizable: true,
			socketPlacement: 'header',
			active: node.meta.enabled,
			enabled: node.meta.enabled,
			canDisable: node.meta.can_be_disabled,
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
	let stateNodesByGraphId = $derived(
		new Map(stateNodes.map((stateNode) => [String(stateNode.node_id), stateNode]))
	);

	const collectVisibleProcessorIds = (node: UiNodeDto, processorIds: Set<string>): void => {
		if (node.user_item_kind === PROCESSOR_ITEM_KIND) {
			processorIds.add(node.uuid);
			return;
		}
		if (
			node.node_type === PROCESSOR_FOLDER_NODE_TYPE &&
			node.meta.presentation?.collapsed === true
		) {
			return;
		}
		for (const childId of node.children) {
			const child = session?.graph.state.nodesById.get(childId);
			if (isProcessorTreeNode(child)) {
				collectVisibleProcessorIds(child, processorIds);
			}
		}
	};

	let visibleProcessorIds = $derived.by(() => {
		const processorIds = new Set<string>();
		for (const stateId of settledVisibleStateIds) {
			const stateNode = stateNodesByGraphId.get(stateId);
			if (!stateNode || stateNode.meta.presentation?.collapsed === true) {
				continue;
			}
			const processorManager = declaredChild(stateNode, PROCESSORS_DECL_ID);
			if (!processorManager) {
				continue;
			}
			for (const childId of processorManager.children) {
				const child = session?.graph.state.nodesById.get(childId);
				if (isProcessorTreeNode(child)) {
					collectVisibleProcessorIds(child, processorIds);
				}
			}
		}
		return [...processorIds].sort();
	});
	let visibleProcessorDemandKey = $derived(visibleProcessorIds.join('\u0000'));

	$effect(() => {
		publishProcessorOverviewDemand(
			visibleProcessorDemandKey === '' ? [] : visibleProcessorDemandKey.split('\u0000')
		);
	});
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
						color: sourceSelected ? '#66cc00' : targetSelected ? '#58a6ff' : undefined,
						active: sourceState.meta.enabled
					}
				];
			});
		});
	});
	const statechartDocumentView = new StatechartDocumentView();
	let graphDocument = $derived(
		statechartDocumentView.update({ states: graphNodes, transitions: graphEdges })
	);
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
	let contextMenuAnchor = $derived.by((): ContextMenuAnchor => ({
		kind: 'point',
		x: contextMenuX,
		y: contextMenuY
	}));

	const panelOwnsFocus = (): boolean =>
		panelRoot !== null &&
		document.activeElement !== null &&
		panelRoot.contains(document.activeElement);

	const selectGraphItems = (nodeIds: string[], edgeIds: string[]): void => {
		if (!session) {
			return;
		}
		const selectedStates = nodeIds
			.map((nodeId) => Number(nodeId))
			.filter((nodeId) => Number.isSafeInteger(nodeId) && stateNodeIds.has(nodeId));
		const selectedTransitions = edgeIds
			.map((edgeId) => Number(edgeId))
			.filter((nodeId) => Number.isSafeInteger(nodeId) && transitionNodeIds.has(nodeId));
		const hierarchyNodeIds = [...selectedStates, ...selectedTransitions];
		if (hierarchyNodeIds.length === 0) {
			if (manager) {
				session.selectNodes([manager.node_id], 'REPLACE');
			} else {
				session.clearSelection();
			}
			return;
		}
		session.selectNodes(hierarchyNodeIds, 'REPLACE');
	};

	const setAutoWire = (enabled: boolean): void => {
		autoWire = enabled;
		writePanelPersistedState(props.panelApi, { autoWire });
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
			const width = graphNodeWidth(node);
			const height = graphNodeHeight(node);
			const firstX = Math.floor(
				(node.position.x - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const lastX = Math.floor(
				(node.position.x + width + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const firstY = Math.floor(
				(node.position.y - STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
			);
			const lastY = Math.floor(
				(node.position.y + height + STATE_PLACEMENT_CLEARANCE_REM) / STATE_PLACEMENT_INDEX_CELL_REM
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
				rectanglesOverlap(candidate, node.position, graphNodeWidth(node), graphNodeHeight(node))
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
					...item.initial_params,
					initialParam(POSITION_DECL_ID, {
						kind: 'vec2',
						value: [position.x, position.y]
					})
				]
			}
		);
		if (result.selectWhenCreated && result.createdNodeId !== null) {
			session?.selectNode(result.createdNodeId, 'REPLACE');
		}
	};

	const renameState = async (nodeId: string, label: string): Promise<void> => {
		if (!session) {
			return;
		}
		const stateId = Number(nodeId);
		const state = session.graph.state.nodesById.get(stateId);
		if (!Number.isSafeInteger(stateId) || state?.node_type !== STATE_NODE_TYPE) {
			return;
		}
		const success = await sendPatchMetaIntent(stateId, { label });
		if (!success) {
			throw new Error(`failed to rename ${state.meta.label}`);
		}
	};

	const setStateCollapsed = async (nodeId: string, collapsed: boolean): Promise<void> => {
		if (!session) {
			return;
		}
		const stateId = Number(nodeId);
		const state = session.graph.state.nodesById.get(stateId);
		if (!Number.isSafeInteger(stateId) || state?.node_type !== STATE_NODE_TYPE) {
			return;
		}
		const success = await sendPatchMetaIntent(stateId, {
			presentation: {
				...(state.meta.presentation ?? {}),
				collapsed
			}
		});
		if (!success) {
			throw new Error(`failed to ${collapsed ? 'collapse' : 'expand'} ${state.meta.label}`);
		}
	};

	const setStateEnabled = async (nodeId: string, enabled: boolean): Promise<void> => {
		if (!session) {
			return;
		}
		const stateId = Number(nodeId);
		const state = session.graph.state.nodesById.get(stateId);
		if (
			!Number.isSafeInteger(stateId) ||
			state?.node_type !== STATE_NODE_TYPE ||
			!state.meta.can_be_disabled
		) {
			return;
		}
		const success = await sendPatchMetaIntent(stateId, { enabled });
		if (!success) {
			throw new Error(`failed to ${enabled ? 'enable' : 'disable'} ${state.meta.label}`);
		}
	};

	let contextMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!manager || manager.creatable_user_items.length === 0) {
			return [];
		}
		return buildCreatableItemMenu(manager.creatable_user_items, (item) =>
			createState(item, contextMenuWorldPosition)
		);
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

	const openGraphNodeContextMenu = (event: MouseEvent, nodeId: string): void => {
		const parsedNodeId = Number(nodeId);
		if (!Number.isSafeInteger(parsedNodeId)) {
			return;
		}
		openNodeContextMenu(parsedNodeId, event.clientX, event.clientY);
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
			const positionParameter = parameterChild(stateNode, POSITION_DECL_ID);
			if (positionParameter?.data.kind !== 'parameter' || positionParameter.data.param.read_only) {
				return [];
			}
			return [
				{
					move,
					stateNode,
					positionParameterId: positionParameter.node_id,
					positionBehaviour: positionParameter.data.param.event_behaviour
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
				writableMoves.map(({ move, positionParameterId, positionBehaviour }) =>
					session.sendIntent({
						kind: 'setParam',
						node: positionParameterId,
						value: { kind: 'vec2', value: [move.position.x, move.position.y] },
						behaviour: positionBehaviour
					})
				)
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
		const sizeParameter = parameterChild(stateNode, SIZE_DECL_ID);
		if (
			sizeParameter?.data.kind !== 'parameter' ||
			sizeParameter.data.param.read_only ||
			!sizeParameter.meta.can_be_disabled
		) {
			throw new Error('State cannot persist canvas dimensions');
		}

		const editSession = createUiEditSession(
			resize.mode === 'custom'
				? `Resize ${stateNode.meta.label}`
				: `Auto-size ${stateNode.meta.label}`,
			'state-size'
		);
		await editSession.begin();
		if (!editSession.active) {
			throw new Error('another edit session is already active');
		}
		try {
			const intents =
				resize.mode === 'custom'
					? [
							session.sendIntent({
								kind: 'setParam',
								node: sizeParameter.node_id,
								value: { kind: 'vec2', value: [resize.size.width, resize.size.height] },
								behaviour: sizeParameter.data.param.event_behaviour
							}),
							session.sendIntent({
								kind: 'patchMeta',
								node: sizeParameter.node_id,
								patch: { enabled: true }
							})
						]
					: [
							session.sendIntent({
								kind: 'patchMeta',
								node: sizeParameter.node_id,
								patch: { enabled: false }
							})
						];
			await Promise.all(intents);
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
		const processorOverviewHeartbeat = setInterval(
			() => publishProcessorOverviewDemand(visibleProcessorIds),
			PROCESSOR_OVERVIEW_HEARTBEAT_MS
		);
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
			clearInterval(processorOverviewHeartbeat);
			if (processorVisibilitySettleTimer !== null) {
				clearTimeout(processorVisibilitySettleTimer);
				processorVisibilitySettleTimer = null;
			}
			publishProcessorOverviewDemand([]);
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

{#snippet graphToolbarContent()}
	{#if manager}
		<GraphToolbarActions
			{autoWire}
			onAutoWireChange={setAutoWire}
			addNode={manager}
			onCreateItem={(item) => createState(item, graphCanvas?.viewportCenter())} />
	{/if}
{/snippet}

<section
	bind:this={panelRoot}
	class="state-machine-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphCanvas?.focus()}>
	<GraphCanvas
		bind:this={graphCanvas}
		{graphDocument}
		{selectedNodeIds}
		{selectedEdgeIds}
		onGraphSelectionChange={selectGraphItems}
		onNodesMove={persistNodePositions}
		onNodeResize={persistNodeResize}
		onNodeRename={renameState}
		onNodeCollapsedChange={setStateCollapsed}
		onVisibleNodeIdsChange={settleVisibleStateIds}
		onNodeEnabledChange={setStateEnabled}
		onConnect={connectStates}
		nodeContent={stateNodeContent}
		onNodeContextMenu={openGraphNodeContextMenu}
		onBackgroundContextMenu={openContextMenu}
		routeEdgesAroundNodes={autoWire}
		{initialCamera}
		onCameraChange={persistCamera}
		toolbarEnd={manager ? graphToolbarContent : undefined}
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
</style>
