<script lang="ts">
	import { ManagerListPanel, type UiNodeDto } from 'golden_ui';
	import ProcessorItem from './ProcessorItem.svelte';

	let { manager }: { manager: UiNodeDto } = $props();
	const isProcessorTreeNode = (node: UiNodeDto | null): node is UiNodeDto =>
		Boolean(
			node &&
			(node.user_item_kind === 'state_processor' || node.node_type === 'state_processor_folder')
		);
</script>

<div class="processor-manager">
	<ManagerListPanel
		managerNode={manager}
		isTreeNode={isProcessorTreeNode}
		canRenderNodeChildren={(node) => node.node_type === 'state_processor_folder'}
		initiallyExpandedDepth={3}
		rowSupplementComponent={ProcessorItem}
		searchPlaceholder="Search processors..."
		emptyMessage="No processors."
		addButtonTitle="Add processor" />
</div>

<style>
	.processor-manager {
		display: flex;
		flex-direction: column;
		min-block-size: 0;
		block-size: 100%;
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--gc-color-background) 58%, transparent);
		overflow: hidden;
	}
</style>
