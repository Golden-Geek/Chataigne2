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
	import { collectOutlinerAncestorNodeIds } from './navigation';
	import TreeRows from './TreeRows.svelte';
	import type { NodeSelectionInput } from './node-selection';
	import {
		canDragOutlinerNode,
		resolveOutlinerDropTarget,
		type OutlinerDropTarget,
		type OutlinerDropZone
	} from './drag-drop';
	import { createOutlinerProjection } from './visible-rows';

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

	export const setNodeExpanded = (nodeId: NodeId, expanded: boolean): void => {
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
	export const searchNodes = (text: string) => {
		query = text;
	};
	let treeElement = $state<HTMLDivElement | null>(null);
	let virtualRows = $state<ReturnType<typeof TreeRows>>();
	export const navigateTree = (key: string, input?: Partial<NodeSelectionInput>) =>
		virtualRows?.navigate(key, input);
	export const enterTree = () => virtualRows?.enter();
	export const focusTreeNode = (id: NodeId) => virtualRows?.focusNode(id);
	let activeDragNodeId = $state<NodeId | null>(null);
	let dropTarget = $state<OutlinerDropTarget | null>(null);
	let moveInFlight = $state(false);

	let rootNode = $derived(mainGraphState?.nodesById.get(mainGraphState?.rootId ?? 0) ?? null);
	let normalizedQuery = $derived(query.trim().toLowerCase());
	const projectRows = createOutlinerProjection();
	let visibleRows = $derived.by(() => {
		if (!mainGraphState) {
			projectRows.clear();
			return [];
		}
		const matchesQuery = normalizedQuery
			? (candidate: UiNodeDto): boolean =>
					`${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type}`
						.toLowerCase()
						.includes(normalizedQuery)
			: undefined;
		return projectRows({
			graph: mainGraphState,
			opennessByNodeId,
			autoExpandAncestorNodeIds,
			initiallyExpandedDepth: 3,
			matchesQuery
		});
	});

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
		if (normalizedQuery.length === 0) {
			return true;
		}
		const haystack =
			`${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type}`.toLowerCase();
		return haystack.includes(normalizedQuery);
	};

	let revealedSelection: string | null = null;
	$effect(() => {
		const node = selectedNodeId === null ? null : mainGraphState?.nodesById.get(selectedNodeId);
		const identity = node ? `${node.node_id}:${node.uuid}` : null;
		if (identity === revealedSelection) return;
		revealedSelection = identity;
		if (node)
			untrack(() => {
				const next = { ...opennessByNodeId };
				for (const ancestor of autoExpandAncestorNodeIds) next[String(ancestor)] = true;
				persistOpennessByNodeId(next);
			});
	});
	$effect(() => {
		query;
		if (!treeElement || selectedNodeId === null) {
			return;
		}
		const target = selectedNodeId;
		void tick().then(() => {
			if (target === selectedNodeId) virtualRows?.reveal(target);
		});
	});
</script>

{#if rootNode}
	<div class="outliner-panel">
		<div class="outliner-header">
			<input
				type="text"
				placeholder="Search..."
				class="outliner-search"
				bind:value={query}
				onkeydown={(event) => {
					if (event.key === 'ArrowDown') {
						event.preventDefault();
						void enterTree();
					}
				}} />
		</div>
		<div
			class="outliner-content"
			bind:this={treeElement}
			role="tree"
			aria-multiselectable="true"
			aria-label="Project tree"
			tabindex="-1">
			<TreeRows
				bind:this={virtualRows}
				rows={visibleRows}
				{selectedNodeId}
				onExpand={setNodeExpanded}
				scroller={treeElement}>
				{#snippet item(row)}
					<div class="outliner-virtual-row" style:padding-inline-start={`${row.indent * 0.7}rem`}>
						<OutlinerItem
							nodeId={row.nodeId}
							siblingParentId={row.parentId}
							level={row.level}
							initiallyExpandedDepth={3}
							renderNodeChildren={false}
							projectedVisible={true}
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
				{/snippet}
			</TreeRows>
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

	.outliner-virtual-row {
		box-sizing: border-box;
		inline-size: 100%;
	}
</style>
