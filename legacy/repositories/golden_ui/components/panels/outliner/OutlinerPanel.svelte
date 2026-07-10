<script lang="ts">
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from '../../../dockview/panel-persistence';
	import type { PanelProps, PanelState } from '../../../dockview/panel-types';
	import type { NodeId, UiNodeDto } from '../../../types';
	import { appState } from '../../../store/workbench.svelte';
	import { sendMoveNodeIntent } from '../../../store/ui-intents';
	import { tick, untrack } from 'svelte';
	import OutlinerItem from './OutlinerItem.svelte';
	import { collectOutlinerAncestorNodeIds, scrollOutlinerNodeIntoView } from './navigation';
	import {
		canDragOutlinerNode,
		resolveOutlinerDropTarget,
		type OutlinerDropTarget,
		type OutlinerDropZone
	} from './drag-drop';

	let { panelApi, panelId, panelType, title, params }: PanelProps = $props();
	let panel = $state<PanelState>({
		panelId: '',
		panelType: '',
		title: '',
		params: {}
	});

	interface OutlinerPersistedState {
		opennessByNodeId?: Record<string, boolean>;
	}

	const isRecord = (value: unknown): value is Record<string, unknown> =>
		typeof value === 'object' && value !== null && !Array.isArray(value);

	const sanitizeOpennessByNodeId = (value: unknown): Record<string, boolean> => {
		if (!isRecord(value)) {
			return {};
		}

		const nextEntries: Array<[string, boolean]> = [];
		for (const [rawNodeId, rawExpanded] of Object.entries(value)) {
			const nodeId = Number(rawNodeId);
			if (!Number.isInteger(nodeId) || nodeId < 0 || typeof rawExpanded !== 'boolean') {
				continue;
			}
			nextEntries.push([String(nodeId), rawExpanded]);
		}
		return Object.fromEntries(nextEntries);
	};

	const sameOpennessByNodeId = (
		left: Record<string, boolean>,
		right: Record<string, boolean>
	): boolean => {
		const leftEntries = Object.entries(left);
		const rightEntries = Object.entries(right);
		if (leftEntries.length !== rightEntries.length) {
			return false;
		}
		for (const [nodeId, expanded] of leftEntries) {
			if (right[nodeId] !== expanded) {
				return false;
			}
		}
		return true;
	};

	let opennessByNodeId = $state<Record<string, boolean>>({});

	const applyPersistedState = (nextParams: PanelState['params']): void => {
		const persistedState = readPanelPersistedState<OutlinerPersistedState>(nextParams);
		const nextOpennessByNodeId = sanitizeOpennessByNodeId(persistedState.opennessByNodeId);
		const currentOpennessByNodeId = untrack(() => opennessByNodeId);
		if (sameOpennessByNodeId(currentOpennessByNodeId, nextOpennessByNodeId)) {
			return;
		}
		opennessByNodeId = nextOpennessByNodeId;
	};

	const persistOpennessByNodeId = (nextOpennessByNodeId: Record<string, boolean>): void => {
		const currentOpennessByNodeId = untrack(() => opennessByNodeId);
		if (sameOpennessByNodeId(currentOpennessByNodeId, nextOpennessByNodeId)) {
			return;
		}
		opennessByNodeId = nextOpennessByNodeId;
		writePanelPersistedState(panelApi, {
			opennessByNodeId: nextOpennessByNodeId
		});
	};

	const setNodeExpanded = (nodeId: NodeId, expanded: boolean): void => {
		const nodeKey = String(nodeId);
		if (opennessByNodeId[nodeKey] === expanded) {
			return;
		}
		persistOpennessByNodeId({
			...opennessByNodeId,
			[nodeKey]: expanded
		});
	};

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
		applyPersistedState(params);
	});

	export const setPanelState = (next: PanelState): void => {
		panel = next;
		applyPersistedState(next.params);
	};

	let mainGraphState = $derived(appState.session?.graph.state);
	let selectedNodeId = $derived(appState.session?.selectedNodeId ?? null);
	let autoExpandAncestorNodeIds = $derived.by(() =>
		collectOutlinerAncestorNodeIds(mainGraphState ?? null, selectedNodeId)
	);
	let query = $state('');
	let treeElement = $state<HTMLDivElement | null>(null);
	let activeDragNodeId = $state<NodeId | null>(null);
	let dropTarget = $state<OutlinerDropTarget | null>(null);
	let moveInFlight = $state(false);

	let rootNode = $derived(mainGraphState?.nodesById.get(mainGraphState?.rootId ?? 0) ?? null);

	const canDragNode = (candidate: UiNodeDto): boolean =>
		canDragOutlinerNode(mainGraphState ?? null, candidate);

	const clearDragState = (): void => {
		activeDragNodeId = null;
		dropTarget = null;
	};

	const resolveDropZone = (event: DragEvent, hoverNodeId: NodeId): OutlinerDropZone => {
		if (mainGraphState?.rootId === hoverNodeId) {
			return 'inside';
		}
		const row = event.currentTarget;
		if (!(row instanceof HTMLElement)) {
			return 'inside';
		}
		const bounds = row.getBoundingClientRect();
		const offsetY = event.clientY - bounds.top;
		const upperThreshold = bounds.height * 0.3;
		const lowerThreshold = bounds.height * 0.7;
		if (offsetY <= upperThreshold) {
			return 'before';
		}
		if (offsetY >= lowerThreshold) {
			return 'after';
		}
		return 'inside';
	};

	const handleNodeDragStart = (node: UiNodeDto): void => {
		if (!canDragNode(node) || moveInFlight) {
			clearDragState();
			return;
		}
		activeDragNodeId = node.node_id;
		dropTarget = null;
	};

	const handleNodeDragEnd = (): void => {
		if (moveInFlight) {
			return;
		}
		clearDragState();
	};

	const handleNodeDragOver = (hoverNode: UiNodeDto, event: DragEvent): void => {
		if (moveInFlight || activeDragNodeId === null) {
			dropTarget = null;
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			mainGraphState ?? null,
			activeDragNodeId,
			hoverNode.node_id,
			resolveDropZone(event, hoverNode.node_id)
		);
		if (!nextDropTarget) {
			dropTarget = null;
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
		dropTarget = nextDropTarget;
	};

	const handleNodeDrop = async (hoverNode: UiNodeDto, event: DragEvent): Promise<void> => {
		const sourceNodeId = activeDragNodeId;
		if (moveInFlight || sourceNodeId === null) {
			clearDragState();
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			mainGraphState ?? null,
			sourceNodeId,
			hoverNode.node_id,
			resolveDropZone(event, hoverNode.node_id)
		);
		clearDragState();
		if (!nextDropTarget) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		moveInFlight = true;
		try {
			await sendMoveNodeIntent(
				sourceNodeId,
				nextDropTarget.newParentId,
				nextDropTarget.newPrevSiblingId ?? undefined
			);
		} finally {
			moveInFlight = false;
			clearDragState();
		}
	};

	$effect(() => {
		if (!mainGraphState || mainGraphState.rootId === null) {
			clearDragState();
			return;
		}

		let didPrune = false;
		const nextEntries: Array<[string, boolean]> = [];
		for (const [nodeIdKey, expanded] of Object.entries(opennessByNodeId)) {
			const nodeId = Number(nodeIdKey) as NodeId;
			if (!mainGraphState.nodesById.has(nodeId)) {
				didPrune = true;
				continue;
			}
			nextEntries.push([nodeIdKey, expanded]);
		}

		if (!didPrune) {
			return;
		}

		persistOpennessByNodeId(Object.fromEntries(nextEntries));
	});

	const nodeFilter = (candidate: UiNodeDto): boolean => {
		const normalizedQuery = query.trim().toLowerCase();
		if (normalizedQuery.length === 0) {
			return true;
		}
		const haystack =
			`${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type}`.toLowerCase();
		return haystack.includes(normalizedQuery);
	};

	$effect(() => {
		query;
		if (!treeElement || selectedNodeId === null) {
			return;
		}

		let cancelled = false;
		let frameId: number | null = null;
		let attempts = 0;
		const maxAttempts = 8;

		const revealSelectedNode = (): void => {
			if (cancelled) {
				return;
			}
			if (scrollOutlinerNodeIntoView(treeElement, selectedNodeId)) {
				return;
			}
			if (attempts >= maxAttempts || typeof requestAnimationFrame === 'undefined') {
				return;
			}
			attempts += 1;
			frameId = requestAnimationFrame(revealSelectedNode);
		};

		void tick().then(() => {
			revealSelectedNode();
		});

		return () => {
			cancelled = true;
			if (frameId !== null && typeof cancelAnimationFrame !== 'undefined') {
				cancelAnimationFrame(frameId);
			}
		};
	});
</script>

{#if rootNode}
	<div class="outliner-panel">
		<div class="outliner-header">
			<input type="text" placeholder="Search..." class="outliner-search" bind:value={query} />
		</div>
		<div class="outliner-content" bind:this={treeElement}>
			<div class="outliner-tree">
				<OutlinerItem
					node={rootNode}
					initiallyExpandedDepth={3}
					{autoExpandAncestorNodeIds}
					{opennessByNodeId}
					onNodeOpennessChange={setNodeExpanded}
					{nodeFilter}
					nodeDraggable={canDragNode}
					{activeDragNodeId}
					{dropTarget}
					onNodeDragStart={handleNodeDragStart}
					onNodeDragOver={handleNodeDragOver}
					onNodeDrop={handleNodeDrop}
					onNodeDragEnd={handleNodeDragEnd} />
			</div>
		</div>
	</div>
{/if}

<style>
	.outliner-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}
	.outliner-header {
		box-sizing: border-box;
	}

	.outliner-search {
		width: 100%;
		padding: 0.25rem;
		box-sizing: border-box;
	}
	.outliner-content {
		flex: 1;
		overflow: auto;
		scrollbar-gutter: stable;
		padding: 0.5rem;
	}
</style>
