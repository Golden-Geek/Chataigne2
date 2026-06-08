<script lang="ts">
	import { onMount } from 'svelte';
	import {
		GraphCanvas,
		type GraphEdge,
		type GraphNode,
		type GraphNodePosition
	} from 'golden_alchemist_ui';
	import type { PanelProps, PanelState, UiNodeDto } from 'golden_ui';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';

	const MANAGER_NODE_TYPE = 'state_machine_manager';
	const STATE_NODE_TYPE = 'state';
	const POSITION_X_DECL_ID = 'x';
	const POSITION_Y_DECL_ID = 'y';
	const DEFAULT_COLUMNS = 4;
	const DEFAULT_X_GAP_REM = 16;
	const DEFAULT_Y_GAP_REM = 7;
	const graphEdges: GraphEdge[] = [];

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

	let graphNodes = $derived.by((): GraphNode[] =>
		stateNodes.map((node, index) => ({
			id: String(node.node_id),
			label: node.meta.label,
			subtitle: 'State',
			...graphPosition(node, index),
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

	const persistNodePosition = async (
		nodeId: string,
		position: GraphNodePosition
	): Promise<void> => {
		if (!session) {
			return;
		}
		const stateNode = session.graph.state.nodesById.get(Number(nodeId));
		if (!stateNode || stateNode.node_type !== STATE_NODE_TYPE) {
			return;
		}
		const xParameter = parameterChild(stateNode, POSITION_X_DECL_ID);
		const yParameter = parameterChild(stateNode, POSITION_Y_DECL_ID);
		if (
			xParameter?.data.kind !== 'parameter' ||
			yParameter?.data.kind !== 'parameter' ||
			xParameter.data.param.read_only ||
			yParameter.data.param.read_only
		) {
			return;
		}

		const clientEditId = `state-position-${stateNode.node_id}-${Date.now()}`;
		await session.sendIntent({
			kind: 'beginEdit',
			client_edit_id: clientEditId,
			label: `Move ${stateNode.meta.label}`
		});
		try {
			await Promise.all([
				session.sendIntent({
					kind: 'setParam',
					node: xParameter.node_id,
					value: { kind: 'float', value: position.x },
					behaviour: xParameter.data.param.event_behaviour
				}),
				session.sendIntent({
					kind: 'setParam',
					node: yParameter.node_id,
					value: { kind: 'float', value: position.y },
					behaviour: yParameter.data.param.event_behaviour
				})
			]);
		} finally {
			await session.sendIntent({ kind: 'endEdit', client_edit_id: clientEditId });
		}
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
		};
	});
</script>

<section
	bind:this={panelRoot}
	class="state-machine-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphCanvas?.focus()}>
	<GraphCanvas
		bind:this={graphCanvas}
		nodes={graphNodes}
		edges={graphEdges}
		{selectedNodeIds}
		onSelectionChange={selectNodes}
		onNodeMove={(nodeId, position) => void persistNodePosition(nodeId, position)}
		{emptyLabel} />
</section>

<style>
	.state-machine-panel {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
	}
</style>
