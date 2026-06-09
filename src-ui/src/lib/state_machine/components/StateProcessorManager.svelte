<script lang="ts">
	import {
		NodeAddButton,
		OutlinerItem,
		canDragOutlinerNode,
		resolveOutlinerDropTarget,
		type NodeId,
		type OutlinerDropTarget,
		type OutlinerDropZone,
		type UiNodeDto
	} from 'golden_ui';
	import { sendMoveNodeIntent } from 'golden_ui/store/ui-intents';
	import { appState } from 'golden_ui/store/workbench.svelte';

	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PROCESSOR_FOLDER_NODE_TYPE = 'state_processor_folder';

	let { manager }: { manager: UiNodeDto } = $props();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveManager = $derived(graph?.nodesById.get(manager.node_id) ?? manager);
	let activeDragNodeId = $state<NodeId | null>(null);
	let dropTarget = $state<OutlinerDropTarget | null>(null);
	let moveInFlight = $state(false);
	let isRootDropActive = $derived(dropTarget?.hoverNodeId === liveManager.node_id);

	const isProcessorTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		Boolean(
			node &&
			(node.user_item_kind === PROCESSOR_ITEM_KIND || node.node_type === PROCESSOR_FOLDER_NODE_TYPE)
		);

	let processorNodes = $derived(
		liveManager.children
			.map((nodeId) => graph?.nodesById.get(nodeId) ?? null)
			.filter(isProcessorTreeNode)
	);

	const canRenderChildren = (node: UiNodeDto): boolean =>
		node.node_type === PROCESSOR_FOLDER_NODE_TYPE;

	const canDragNode = (node: UiNodeDto): boolean => canDragOutlinerNode(graph, node);

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
		const ratio = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
		return ratio <= 0.3 ? 'before' : ratio >= 0.7 ? 'after' : 'inside';
	};

	const handleNodeDragStart = (node: UiNodeDto): void => {
		if (!canDragNode(node) || moveInFlight) {
			clearDragState();
			return;
		}
		activeDragNodeId = node.node_id;
		dropTarget = null;
	};

	const handleNodeDragOver = (hoverNode: UiNodeDto, event: DragEvent): void => {
		if (moveInFlight || activeDragNodeId === null) {
			dropTarget = null;
			return;
		}
		const next = resolveOutlinerDropTarget(
			graph,
			activeDragNodeId,
			hoverNode.node_id,
			resolveDropZone(event)
		);
		if (!next) {
			dropTarget = null;
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
		dropTarget = next;
	};

	const commitDrop = async (
		sourceNodeId: NodeId,
		next: OutlinerDropTarget | null,
		event: DragEvent
	): Promise<void> => {
		clearDragState();
		if (!next) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		moveInFlight = true;
		try {
			await sendMoveNodeIntent(sourceNodeId, next.newParentId, next.newPrevSiblingId ?? undefined);
		} finally {
			moveInFlight = false;
			clearDragState();
		}
	};

	const handleNodeDrop = async (hoverNode: UiNodeDto, event: DragEvent): Promise<void> => {
		const sourceNodeId = activeDragNodeId;
		if (sourceNodeId === null || moveInFlight) {
			clearDragState();
			return;
		}
		await commitDrop(
			sourceNodeId,
			resolveOutlinerDropTarget(graph, sourceNodeId, hoverNode.node_id, resolveDropZone(event)),
			event
		);
	};

	const handleRootDragOver = (event: DragEvent): void => {
		if (
			moveInFlight ||
			activeDragNodeId === null ||
			(event.target instanceof Element && event.target.closest('.outliner-item-content'))
		) {
			return;
		}
		const next = resolveOutlinerDropTarget(graph, activeDragNodeId, liveManager.node_id, 'inside');
		if (!next) {
			dropTarget = null;
			return;
		}
		event.preventDefault();
		dropTarget = next;
	};

	const handleRootDrop = async (event: DragEvent): Promise<void> => {
		const sourceNodeId = activeDragNodeId;
		if (
			sourceNodeId === null ||
			moveInFlight ||
			(event.target instanceof Element && event.target.closest('.outliner-item-content'))
		) {
			return;
		}
		await commitDrop(
			sourceNodeId,
			resolveOutlinerDropTarget(graph, sourceNodeId, liveManager.node_id, 'inside'),
			event
		);
	};
</script>

<div class="processor-manager">
	<div class="processor-manager-header">
		<span>Processors</span>
		<NodeAddButton node={liveManager} />
	</div>
	<div
		class="processor-list"
		class:root-drop-active={isRootDropActive}
		role="presentation"
		ondragover={handleRootDragOver}
		ondrop={(event) => void handleRootDrop(event)}
		ondragleave={() => {
			if (isRootDropActive) {
				dropTarget = null;
			}
		}}>
		{#if processorNodes.length === 0}
			<p class="processor-empty">No processors.</p>
		{:else}
			{#each processorNodes as processorNode (processorNode.node_id)}
				<OutlinerItem
					node={processorNode}
					mode="tree"
					initiallyExpandedDepth={3}
					canRenderNodeChildren={canRenderChildren}
					nodeDraggable={canDragNode}
					{activeDragNodeId}
					{dropTarget}
					onNodeDragStart={handleNodeDragStart}
					onNodeDragOver={handleNodeDragOver}
					onNodeDrop={handleNodeDrop}
					onNodeDragEnd={() => {
						if (!moveInFlight) {
							clearDragState();
						}
					}} />
			{/each}
		{/if}
	</div>
</div>

<style>
	.processor-manager {
		display: flex;
		flex-direction: column;
		min-block-size: 0;
		block-size: 100%;
		/* border: solid 0.06rem color-mix(in srgb, var(--gc-color-panel-outline) 38%, transparent); */
		/* border-radius: 0.38rem; */
		background: color-mix(in srgb, var(--gc-color-background) 58%, transparent);
		overflow: hidden;
	}

	.processor-manager-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-block-size: 1.45rem;
		padding-inline: 0.4rem 0.15rem;
		border-block-end: solid 0.06rem
			color-mix(in srgb, var(--gc-color-panel-outline) 32%, transparent);
		font-size: 0.68rem;
		font-weight: 600;
	}

	.processor-list {
		flex: 1 1 auto;
		min-block-size: 0;
		padding: 0.2rem;
		overflow: auto;
		outline: solid 0.08rem transparent;
		outline-offset: -0.08rem;
	}

	.processor-list.root-drop-active {
		background: color-mix(in srgb, var(--gc-color-selection) 8%, transparent);
		outline-color: color-mix(in srgb, var(--gc-color-selection) 48%, transparent);
	}

	.processor-empty {
		margin: 0;
		padding: 0.35rem;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.68rem;
		text-align: center;
	}
</style>
