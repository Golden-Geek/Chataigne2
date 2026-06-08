<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GraphCanvas,
		type GraphCamera,
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
		type UiCreatableUserItem,
		type UiNodeDto
	} from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { createUiEditSession, sendCreateUserItemIntent } from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import addIcon from 'golden_ui/style/icons/node/add.svg';

	const MANAGER_NODE_TYPE = 'state_machine_manager';
	const STATE_NODE_TYPE = 'state';
	const POSITION_X_DECL_ID = 'x';
	const POSITION_Y_DECL_ID = 'y';
	const WIDTH_DECL_ID = 'width';
	const HEIGHT_DECL_ID = 'height';
	const DEFAULT_COLUMNS = 4;
	const DEFAULT_X_GAP_REM = 16;
	const DEFAULT_Y_GAP_REM = 7;
	const DEFAULT_STATE_WIDTH_REM = 13;
	const DEFAULT_STATE_HEIGHT_REM = 8;
	const CAMERA_PERSIST_DELAY_MS = 150;
	const graphEdges: GraphEdge[] = [];

	interface StateMachinePanelPersistedState {
		camera?: GraphCamera;
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
	} | null = $state(null);
	let session = $derived(appState.session);
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let geometryPersistenceTail = Promise.resolve();
	let pendingCamera: GraphCamera | null = null;
	let cameraPersistenceTimer: ReturnType<typeof setTimeout> | null = null;

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

	const floatParameterValue = (node: UiNodeDto, declId: string): number | null => {
		const parameter = parameterChild(node, declId);
		if (parameter?.data.kind !== 'parameter' || parameter.data.param.value.kind !== 'float') {
			return null;
		}
		return parameter.data.param.value.value;
	};

	const graphPosition = (node: UiNodeDto, index: number): GraphNodePosition => {
		const x = floatParameterValue(node, POSITION_X_DECL_ID);
		const y = floatParameterValue(node, POSITION_Y_DECL_ID);
		if (x === null || y === null || (x === 0 && y === 0)) {
			return {
				x: 2 + (index % DEFAULT_COLUMNS) * DEFAULT_X_GAP_REM,
				y: 2 + Math.floor(index / DEFAULT_COLUMNS) * DEFAULT_Y_GAP_REM
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
			subtitle: 'State',
			...graphPosition(node, index),
			width: graphDimension(node, WIDTH_DECL_ID, DEFAULT_STATE_WIDTH_REM),
			height: graphDimension(node, HEIGHT_DECL_ID, DEFAULT_STATE_HEIGHT_REM),
			resizable: true,
			inputs: [],
			outputs: []
		}))
	);
	let stateNodeIds = $derived(new Set(stateNodes.map((node) => node.node_id)));
	let selectedNodeIds = $derived(
		(session?.selectedNodesIds ?? [])
			.filter((nodeId) => stateNodeIds.has(nodeId))
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

	const createState = async (item: UiCreatableUserItem): Promise<void> => {
		if (!manager) {
			return;
		}
		contextMenuOpen = false;
		const result = await sendCreateUserItemIntent(manager.node_id, item);
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
				submenu: buildCreatableItemMenu(manager.creatable_user_items, createState)
			}
		];
	});

	const openContextMenu = (event: MouseEvent): void => {
		if (contextMenuItems.length === 0) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		contextMenuX = event.clientX;
		contextMenuY = event.clientY;
		contextMenuOpen = true;
		graphCanvas?.focus();
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

<section
	bind:this={panelRoot}
	class="state-machine-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphCanvas?.focus()}
	oncontextmenu={openContextMenu}>
	{#if manager}
		<div class="state-machine-actions">
			<NodeAddButton node={manager} />
		</div>
	{/if}

	<GraphCanvas
		bind:this={graphCanvas}
		nodes={graphNodes}
		edges={graphEdges}
		{selectedNodeIds}
		onSelectionChange={selectNodes}
		onNodesMove={persistNodePositions}
		onNodeResize={persistNodeResize}
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
	}

	.state-machine-actions {
		position: absolute;
		inset-block-start: 0.7rem;
		inset-inline-start: 0.7rem;
		z-index: 20;
		display: flex;
		padding: 0.28rem;
		border: solid 0.06rem rgb(from var(--gc-color-panel-outline) r g b / 0.55);
		border-radius: 0.5rem;
		background: rgb(from var(--gc-color-background) r g b / 0.84);
		backdrop-filter: blur(0.5rem);
	}
</style>
