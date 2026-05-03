<script lang="ts">
	import { tick } from 'svelte';
	import OutlinerItem from '../../golden_ui/components/panels/outliner/OutlinerItem.svelte';
	import {
		canDragOutlinerNode,
		resolveOutlinerDropTarget,
		type OutlinerDropTarget,
		type OutlinerDropZone
	} from '../../golden_ui/components/panels/outliner/drag-drop';
	import { scrollOutlinerNodeIntoView } from '../../golden_ui/components/panels/outliner/navigation';
	import NodeAddButton from '../../golden_ui/components/common/NodeAddButton.svelte';
	import type { PanelProps, PanelState } from '../../golden_ui/dockview/panel-types';
	import type { GraphState } from '../../golden_ui/store/graph.svelte';
	import { sendMoveNodeIntent } from '../../golden_ui/store/ui-intents';
	import { appState } from '../../golden_ui/store/workbench.svelte';
	import type { NodeId, UiNodeDto } from '../../golden_ui/types';
	import ModuleItem from './ModuleItem.svelte';

	let { panelId, panelType, title, params }: PanelProps = $props();

	let panel = $state<PanelState>({
		panelId: '',
		panelType: '',
		title: '',
		params: {}
	});

	const MODULE_MANAGER_NODE_TYPE = 'module_manager';
	const MODULE_USER_ITEM_KIND = 'module';
	const MODULE_FOLDER_NODE_TYPE = 'folder';

	const isModuleFolderNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		Boolean(candidate && candidate.node_type === MODULE_FOLDER_NODE_TYPE);

	const isModuleLeafNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		Boolean(
			candidate &&
			candidate.user_item_kind === MODULE_USER_ITEM_KIND &&
			candidate.node_type !== MODULE_FOLDER_NODE_TYPE
		);

	const isModuleTreeNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		isModuleLeafNode(candidate) || isModuleFolderNode(candidate);

	const findModuleManagerRoot = (graph: GraphState | null): UiNodeDto | null => {
		if (!graph || graph.rootId === null) {
			return null;
		}

		const topLevelNodeIds = graph.childrenById.get(graph.rootId) ?? [];
		for (const childNodeId of topLevelNodeIds) {
			const childNode = graph.nodesById.get(childNodeId) ?? null;
			if (childNode?.node_type === MODULE_MANAGER_NODE_TYPE) {
				return childNode;
			}
		}

		for (const candidate of graph.nodesById.values()) {
			if (candidate.node_type === MODULE_MANAGER_NODE_TYPE) {
				return candidate;
			}
		}

		return null;
	};

	const findVisibleModuleTreeNodeId = (
		graph: GraphState | null,
		nodeId: NodeId | null
	): NodeId | null => {
		if (!graph || nodeId === null) {
			return null;
		}

		let currentNodeId: NodeId | undefined = nodeId;
		while (currentNodeId !== undefined) {
			const currentNode = graph.nodesById.get(currentNodeId) ?? null;
			if (isModuleTreeNode(currentNode)) {
				return currentNodeId;
			}
			currentNodeId = graph.parentById.get(currentNodeId);
		}

		return null;
	};

	const nodeSearchText = (candidate: UiNodeDto): string => {
		return `${candidate.meta.label} ${candidate.meta.short_name} ${candidate.node_type}`.toLowerCase();
	};

	const canRenderModuleChildren = (candidate: UiNodeDto): boolean => {
		return !isModuleLeafNode(candidate);
	};

	const isModuleRowTarget = (target: EventTarget | null): boolean => {
		return target instanceof Element && target.closest('.outliner-item-content') !== null;
	};

	$effect(() => {
		panel = {
			panelId,
			panelType,
			title,
			params
		};
	});

	export const setPanelState = (next: PanelState): void => {
		panel = next;
	};

	let session = $derived(appState.session);
	let mainGraphState = $derived(session?.graph.state ?? null);
	let selectedNodeId = $derived(session?.selectedNodeId ?? null);
	let focusedModuleNodeId = $derived(findVisibleModuleTreeNodeId(mainGraphState, selectedNodeId));
	let moduleManagerNode = $derived(findModuleManagerRoot(mainGraphState));
	let moduleNodes = $derived.by(() => {
		if (!mainGraphState || !moduleManagerNode) {
			return [];
		}

		return moduleManagerNode.children
			.map((childNodeId) => mainGraphState.nodesById.get(childNodeId) ?? null)
			.filter(isModuleTreeNode);
	});
	let query = $state('');
	let treeElement = $state<HTMLDivElement | null>(null);
	let activeDragNodeId = $state<NodeId | null>(null);
	let dropTarget = $state<OutlinerDropTarget | null>(null);
	let moveInFlight = $state(false);
	let isRootDropActive = $derived(
		moduleManagerNode !== null && dropTarget?.hoverNodeId === moduleManagerNode.node_id
	);

	const nodeFilter = (candidate: UiNodeDto): boolean => {
		const normalizedQuery = query.trim().toLowerCase();
		if (normalizedQuery.length === 0) {
			return true;
		}
		return nodeSearchText(candidate).includes(normalizedQuery);
	};

	const selectModuleManager = (): void => {
		if (!moduleManagerNode) {
			return;
		}
		session?.selectNode(moduleManagerNode.node_id, 'REPLACE');
	};

	const canDragNode = (candidate: UiNodeDto): boolean =>
		canDragOutlinerNode(mainGraphState ?? null, candidate);

	const clearDragState = (): void => {
		activeDragNodeId = null;
		dropTarget = null;
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
		if (moveInFlight || sourceNodeId === null) {
			clearDragState();
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			mainGraphState ?? null,
			sourceNodeId,
			hoverNode.node_id,
			resolveDropZone(event)
		);
		await commitDropTarget(sourceNodeId, nextDropTarget, event);
	};

	const handleRootDragOver = (event: DragEvent): void => {
		if (moveInFlight || activeDragNodeId === null || !moduleManagerNode) {
			dropTarget = null;
			return;
		}
		if (isModuleRowTarget(event.target)) {
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			mainGraphState ?? null,
			activeDragNodeId,
			moduleManagerNode.node_id,
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
		if (moveInFlight || sourceNodeId === null || !moduleManagerNode) {
			clearDragState();
			return;
		}
		if (isModuleRowTarget(event.target)) {
			return;
		}
		const nextDropTarget = resolveOutlinerDropTarget(
			mainGraphState ?? null,
			sourceNodeId,
			moduleManagerNode.node_id,
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
		if (!moduleManagerNode || isModuleRowTarget(event.target)) {
			return;
		}
		if (event.button !== 0 && event.button !== 2) {
			return;
		}
		selectModuleManager();
	};

	$effect(() => {
		query;
		if (!treeElement || focusedModuleNodeId === null) {
			return;
		}

		let cancelled = false;
		let frameId: number | null = null;
		let attempts = 0;
		const maxAttempts = 8;

		const revealFocusedModule = (): void => {
			if (cancelled) {
				return;
			}
			if (scrollOutlinerNodeIntoView(treeElement, focusedModuleNodeId)) {
				return;
			}
			if (attempts >= maxAttempts || typeof requestAnimationFrame === 'undefined') {
				return;
			}
			attempts += 1;
			frameId = requestAnimationFrame(revealFocusedModule);
		};

		void tick().then(() => {
			revealFocusedModule();
		});

		return () => {
			cancelled = true;
			if (frameId !== null && typeof cancelAnimationFrame !== 'undefined') {
				cancelAnimationFrame(frameId);
			}
		};
	});

	$effect(() => {
		if (!mainGraphState || !moduleManagerNode) {
			clearDragState();
			return;
		}
		if (activeDragNodeId !== null && !mainGraphState.nodesById.has(activeDragNodeId)) {
			clearDragState();
		}
	});
</script>

<div class="module-panel">
	<div class="module-header">
		<input type="search" placeholder="Search modules..." class="module-search" bind:value={query} />
		{#if moduleManagerNode}
			<div class="module-add-button" title="Add item to Module Manager">
				<NodeAddButton node={moduleManagerNode} />
			</div>
		{/if}
	</div>

	<div
		class="module-content"
		class:root-drop-active={isRootDropActive}
		role="presentation"
		data-node-id={moduleManagerNode?.node_id ?? undefined}
		bind:this={treeElement}
		onpointerdown={handleBackgroundPointerDown}
		ondragover={handleRootDragOver}
		ondrop={(event) => {
			void handleRootDrop(event);
		}}
		ondragleave={handleRootDragLeave}>
		{#if !moduleManagerNode}
			<div class="module-empty">No module manager found in the current graph.</div>
		{:else if moduleNodes.length === 0}
			<div class="module-empty">
				{#if isRootDropActive}
					Drop here to move into Module Manager.
				{:else}
					No modules available.
				{/if}
			</div>
		{:else}
			<div class="module-tree">
				{#each moduleNodes as moduleNode (moduleNode.node_id)}
					<OutlinerItem
						node={moduleNode}
						mode="tree"
						focusedNodeId={focusedModuleNodeId}
						{nodeFilter}
						canRenderNodeChildren={canRenderModuleChildren}
						rowSupplementComponent={ModuleItem}
						nodeDraggable={canDragNode}
						{activeDragNodeId}
						{dropTarget}
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
	.module-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.module-header {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		box-sizing: border-box;
	}

	.module-search {
		flex: 1 1 auto;
		padding: 0.25rem;
		box-sizing: border-box;
	}

	.module-add-button {
		flex: 0 0 auto;
		display: inline-flex;
		align-items: center;
	}

	.module-content {
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

	.module-content.root-drop-active {
		background: rgb(from var(--gc-color-selection) r g b / 0.08);
		outline-color: rgb(from var(--gc-color-selection) r g b / 0.45);
	}

	.module-tree {
		display: flex;
		flex-direction: column;
		gap: 0.1rem;
	}

	.module-empty {
		padding: 0.35rem 0.25rem;
		color: rgb(from var(--gc-color-text) r g b / 0.72);
		font-size: 0.8rem;
	}
</style>
