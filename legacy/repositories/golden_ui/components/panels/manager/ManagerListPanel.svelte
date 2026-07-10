<script lang="ts">
	import { tick, type Component } from 'svelte';
	import NodeAddButton from '../../common/NodeAddButton.svelte';
	import OutlinerItem from '../outliner/OutlinerItem.svelte';
	import {
		canDragOutlinerNode,
		resolveOutlinerDropTarget,
		type OutlinerDropTarget,
		type OutlinerDropZone
	} from '../outliner/drag-drop';
	import { scrollOutlinerNodeIntoView } from '../outliner/navigation';
	import type { GraphState } from '../../../store/graph.svelte';
	import { sendMoveNodeIntent } from '../../../store/ui-intents';
	import { appState } from '../../../store/workbench.svelte';
	import type { NodeId, UiCreatableUserItem, UiNodeDto } from '../../../types';

	type ManagerNodePredicate = (candidate: UiNodeDto | null) => candidate is UiNodeDto;
	type ManagerChildrenPredicate = (candidate: UiNodeDto) => boolean;
	type ManagerNodeSearchText = (candidate: UiNodeDto) => string;

	const defaultSearchText: ManagerNodeSearchText = (candidate) =>
		`${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type} ${candidate.user_item_kind}`.toLowerCase();

	let {
		managerNodeType = null,
		managerNode: explicitManagerNode = null,
		addTargetNode: explicitAddTargetNode = null,
		addItems = undefined,
		searchPlaceholder = 'Search...',
		missingMessage = 'Manager not found in the current graph.',
		emptyMessage = 'No items available.',
		rootDropMessage = 'Drop here to move into this manager.',
		addButtonTitle = 'Add item',
		rowSupplementComponent: RowSupplementComponent = null,
		isTreeNode,
		canRenderNodeChildren = (_candidate: UiNodeDto) => false,
		nodeSearchText = defaultSearchText,
		extraNodeFilter = null,
		initiallyExpandedDepth = 5,
		allowDragDrop = true,
		nodeDraggable = null,
		onCreateItem = undefined,
		onSelectNode = undefined,
		onNodeDragStartData = undefined,
		showSessionSelection = true
	} = $props<{
		managerNodeType?: string | null;
		managerNode?: UiNodeDto | null;
		addTargetNode?: UiNodeDto | null;
		addItems?: readonly UiCreatableUserItem[] | undefined;
		searchPlaceholder?: string;
		missingMessage?: string;
		emptyMessage?: string;
		rootDropMessage?: string;
		addButtonTitle?: string;
		rowSupplementComponent?: Component<{ node: UiNodeDto }> | null;
		isTreeNode: ManagerNodePredicate;
		canRenderNodeChildren?: ManagerChildrenPredicate;
		nodeSearchText?: ManagerNodeSearchText;
		/** Additional predicate ANDed with the search-text match, e.g. a category filter. */
		extraNodeFilter?: ManagerChildrenPredicate | null;
		initiallyExpandedDepth?: number;
		allowDragDrop?: boolean;
		nodeDraggable?: ((candidate: UiNodeDto) => boolean) | null;
		onCreateItem?: (parent: UiNodeDto, item: UiCreatableUserItem) => void | Promise<void>;
		onSelectNode?: (node: UiNodeDto, event: MouseEvent) => void;
		onNodeDragStartData?: (node: UiNodeDto, event: DragEvent) => void;
		showSessionSelection?: boolean;
	}>();

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let selectedNodeId = $derived(session?.selectedNodeId ?? null);
	let query = $state('');
	let treeElement = $state<HTMLDivElement | null>(null);
	let activeDragNodeId = $state<NodeId | null>(null);
	let dropTarget = $state<OutlinerDropTarget | null>(null);
	let moveInFlight = $state(false);

	const findManagerRoot = (graph: GraphState | null): UiNodeDto | null => {
		if (!graph || graph.rootId === null) {
			return null;
		}

		const topLevelNodeIds = graph.childrenById.get(graph.rootId) ?? [];
		for (const childNodeId of topLevelNodeIds) {
			const childNode = graph.nodesById.get(childNodeId) ?? null;
			if (managerNodeType !== null && childNode?.node_type === managerNodeType) {
				return childNode;
			}
		}

		for (const candidate of graph.nodesById.values()) {
			if (managerNodeType !== null && candidate.node_type === managerNodeType) {
				return candidate;
			}
		}

		return null;
	};

	const findVisibleTreeNodeId = (
		graph: GraphState | null,
		nodeId: NodeId | null
	): NodeId | null => {
		if (!graph || nodeId === null) {
			return null;
		}

		let currentNodeId: NodeId | undefined = nodeId;
		while (currentNodeId !== undefined) {
			const currentNode = graph.nodesById.get(currentNodeId) ?? null;
			if (isTreeNode(currentNode)) {
				return currentNodeId;
			}
			currentNodeId = graph.parentById.get(currentNodeId);
		}

		return null;
	};

	let managerNode = $derived(explicitManagerNode ?? findManagerRoot(graphState));
	let addTargetNode = $derived(explicitAddTargetNode ?? managerNode);
	let focusedNodeId = $derived(findVisibleTreeNodeId(graphState, selectedNodeId));
	let itemNodes = $derived.by((): UiNodeDto[] => {
		if (!graphState || !managerNode) {
			return [];
		}

		return managerNode.children
			.map((childNodeId: NodeId) => graphState.nodesById.get(childNodeId) ?? null)
			.filter(isTreeNode);
	});
	let itemNodeIds = $derived(itemNodes.map((itemNode) => itemNode.node_id));
	let isRootDropActive = $derived(
		managerNode !== null && dropTarget?.hoverNodeId === managerNode.node_id
	);

	const nodeFilter = (candidate: UiNodeDto): boolean => {
		if (!isTreeNode(candidate)) {
			return false;
		}
		if (extraNodeFilter && !extraNodeFilter(candidate)) {
			return false;
		}
		const normalizedQuery = query.trim().toLowerCase();
		return (
			normalizedQuery.length === 0 ||
			nodeSearchText(candidate).toLowerCase().includes(normalizedQuery)
		);
	};

	const isManagerRowTarget = (target: EventTarget | null): boolean => {
		return target instanceof Element && target.closest('.outliner-item-content') !== null;
	};

	const clearDragState = (): void => {
		activeDragNodeId = null;
		dropTarget = null;
	};

	const canDragNode = (candidate: UiNodeDto): boolean => {
		return (
			allowDragDrop &&
			(nodeDraggable
				? nodeDraggable(candidate)
				: canDragOutlinerNode(graphState ?? null, candidate))
		);
	};

	const resolveDropZone = (event: DragEvent): OutlinerDropZone => {
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

	const handleNodeDragStart = (node: UiNodeDto, event: DragEvent): void => {
		onNodeDragStartData?.(node, event);
		if (!canDragNode(node) || moveInFlight) {
			clearDragState();
			return;
		}
		activeDragNodeId = node.node_id;
		dropTarget = null;
	};

	const handleNodeDragEnd = (): void => {
		if (!moveInFlight) {
			clearDragState();
		}
	};

	const handleNodeDragOver = (hoverNode: UiNodeDto, event: DragEvent): void => {
		if (!allowDragDrop || moveInFlight || activeDragNodeId === null) {
			dropTarget = null;
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			graphState ?? null,
			activeDragNodeId,
			hoverNode.node_id,
			resolveDropZone(event)
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

	const commitDropTarget = async (
		sourceNodeId: NodeId,
		nextDropTarget: OutlinerDropTarget | null,
		event: DragEvent
	): Promise<void> => {
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

	const handleNodeDrop = async (hoverNode: UiNodeDto, event: DragEvent): Promise<void> => {
		const sourceNodeId = activeDragNodeId;
		if (!allowDragDrop || moveInFlight || sourceNodeId === null) {
			clearDragState();
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			graphState ?? null,
			sourceNodeId,
			hoverNode.node_id,
			resolveDropZone(event)
		);
		await commitDropTarget(sourceNodeId, nextDropTarget, event);
	};

	const handleRootDragOver = (event: DragEvent): void => {
		if (!allowDragDrop || moveInFlight || activeDragNodeId === null || !managerNode) {
			dropTarget = null;
			return;
		}
		if (isManagerRowTarget(event.target)) {
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			graphState ?? null,
			activeDragNodeId,
			managerNode.node_id,
			'inside'
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

	const handleRootDrop = async (event: DragEvent): Promise<void> => {
		const sourceNodeId = activeDragNodeId;
		if (!allowDragDrop || moveInFlight || sourceNodeId === null || !managerNode) {
			clearDragState();
			return;
		}
		if (isManagerRowTarget(event.target)) {
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			graphState ?? null,
			sourceNodeId,
			managerNode.node_id,
			'inside'
		);
		await commitDropTarget(sourceNodeId, nextDropTarget, event);
	};

	const handleRootDragLeave = (event: DragEvent): void => {
		if (!isRootDropActive) {
			return;
		}
		const nextTarget = event.relatedTarget;
		if (nextTarget instanceof Node && event.currentTarget instanceof HTMLElement) {
			if (event.currentTarget.contains(nextTarget)) {
				return;
			}
		}
		dropTarget = null;
	};

	const handleBackgroundPointerDown = (event: PointerEvent): void => {
		if (!managerNode || isManagerRowTarget(event.target)) {
			return;
		}
		if (event.button !== 0 && event.button !== 2) {
			return;
		}
		session?.selectNode(managerNode.node_id, 'REPLACE');
	};

	const createItem = (item: UiCreatableUserItem): void | Promise<void> => {
		if (!addTargetNode || !onCreateItem) {
			return;
		}
		return onCreateItem(addTargetNode, item);
	};

	$effect(() => {
		query;
		if (!treeElement || focusedNodeId === null) {
			return;
		}

		let cancelled = false;
		let frameId: number | null = null;
		let attempts = 0;
		const maxAttempts = 8;

		const revealFocusedNode = (): void => {
			if (cancelled) {
				return;
			}
			if (scrollOutlinerNodeIntoView(treeElement, focusedNodeId)) {
				return;
			}
			if (attempts >= maxAttempts || typeof requestAnimationFrame === 'undefined') {
				return;
			}
			attempts += 1;
			frameId = requestAnimationFrame(revealFocusedNode);
		};

		void tick().then(() => {
			revealFocusedNode();
		});

		return () => {
			cancelled = true;
			if (frameId !== null && typeof cancelAnimationFrame !== 'undefined') {
				cancelAnimationFrame(frameId);
			}
		};
	});

	$effect(() => {
		if (!graphState || !managerNode) {
			clearDragState();
			return;
		}
		if (activeDragNodeId !== null && !graphState.nodesById.has(activeDragNodeId)) {
			clearDragState();
		}
	});
</script>

<div class="manager-list-panel">
	<div class="manager-list-header">
		<input
			type="search"
			placeholder={searchPlaceholder}
			class="manager-list-search"
			bind:value={query} />
		{#if addTargetNode}
			<div class="manager-list-add-button" title={addButtonTitle}>
				<NodeAddButton
					node={addTargetNode}
					items={addItems}
					onCreateItem={onCreateItem ? createItem : undefined} />
			</div>
		{/if}
	</div>

	<div
		class="manager-list-content"
		class:root-drop-active={isRootDropActive}
		role="presentation"
		data-node-id={managerNode?.node_id ?? undefined}
		bind:this={treeElement}
		onpointerdown={handleBackgroundPointerDown}
		ondragover={handleRootDragOver}
		ondrop={(event) => {
			void handleRootDrop(event);
		}}
		ondragleave={handleRootDragLeave}>
		{#if !managerNode}
			<div class="manager-list-empty">{missingMessage}</div>
		{:else if itemNodes.length === 0}
			<div class="manager-list-empty">
				{#if isRootDropActive}
					{rootDropMessage}
				{:else}
					{emptyMessage}
				{/if}
			</div>
		{:else}
			<div class="manager-list-tree">
				{#each itemNodes as itemNode (itemNode.node_id)}
					<OutlinerItem
						node={itemNode}
						siblingNodeIds={itemNodeIds}
						mode="tree"
						{focusedNodeId}
						{nodeFilter}
						{canRenderNodeChildren}
						rowSupplementComponent={RowSupplementComponent}
						nodeDraggable={canDragNode}
						{activeDragNodeId}
						{dropTarget}
						{initiallyExpandedDepth}
						{showSessionSelection}
						{onSelectNode}
						onNodeDragStart={handleNodeDragStart}
						onNodeDragOver={handleNodeDragOver}
						onNodeDrop={handleNodeDrop}
						onNodeDragEnd={handleNodeDragEnd} />
				{/each}
			</div>
		{/if}
	</div>
</div>

<style>
	.manager-list-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.manager-list-header {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		box-sizing: border-box;
	}

	.manager-list-search {
		flex: 1 1 auto;
		padding: 0.25rem;
		box-sizing: border-box;
	}

	.manager-list-add-button {
		flex: 0 0 auto;
		display: inline-flex;
		align-items: center;
	}

	.manager-list-content {
		flex: 1;
		overflow: auto;
		scrollbar-gutter: stable;
		padding: 0.5rem;
		border-radius: 0.45rem;
		transition:
			background-color 0.12s ease,
			outline-color 0.12s ease;
		outline: solid 0.08rem transparent;
	}

	.manager-list-content.root-drop-active {
		background: rgb(from var(--gc-color-selection) r g b / 0.08);
		outline-color: rgb(from var(--gc-color-selection) r g b / 0.45);
	}

	.manager-list-tree {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}

	.manager-list-empty {
		padding: 0.35rem 0.25rem;
		color: rgb(from var(--gc-color-text) r g b / 0.72);
		font-size: 0.8rem;
	}
</style>
