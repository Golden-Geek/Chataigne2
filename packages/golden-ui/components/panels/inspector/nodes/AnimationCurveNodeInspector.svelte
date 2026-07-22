<script lang="ts">
	import { appState } from '../../../../store/workbench.svelte';
	import type { NodeId, UiNodeDto } from '../../../../types';
	import type { NodeInspectorComponentProps } from '../node-inspector-registry';
	import NodeInspector from '../NodeInspector.svelte';
	import SelectNodeButton from '../../../common/SelectNodeButton.svelte';
	import AnimationCurveNodeEditor from '../../../common/AnimationCurveNodeEditor.svelte';

	let { node, level, defaultHeader, defaultContent }: NodeInspectorComponentProps = $props();

	const CURVE_NODE_TYPE = 'animation_curve';
	const KEY_NODE_TYPE = 'animation_curve_key';
	const EASING_NODE_TYPE = 'animation_curve_easing';
	const RANGE_NODE_TYPE = 'animation_curve_range';
	const DECL_EASING = 'easing';
	const DECL_RANGE = 'range';

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let liveNode: UiNodeDto = $derived(session?.graph.state.nodesById.get(node.node_id) ?? node);

	let selected_key_id = $state<NodeId | null>(null);
	let selected_key_ids = $state<NodeId[]>([]);
	let selected_curve_owner_key_ids = $state<NodeId[]>([]);

	const same_node_id_array = (left: NodeId[], right: NodeId[]): boolean => {
		if (left.length !== right.length) {
			return false;
		}
		for (let index = 0; index < left.length; index += 1) {
			if (left[index] !== right[index]) {
				return false;
			}
		}
		return true;
	};

	let key_count_label = $derived.by((): string => {
		if (liveNode.node_type !== CURVE_NODE_TYPE || !graphNodesById) {
			return '0 keys';
		}
		let count = 0;
		for (const child_id of liveNode.children) {
			const child = graphNodesById.get(child_id);
			if (child?.node_type === KEY_NODE_TYPE) {
				count += 1;
			}
		}
		return `${count} key${count === 1 ? '' : 's'}`;
	});

	let editable_curve_range_node = $derived.by((): UiNodeDto | null => {
		if (!graphNodesById || liveNode.node_type !== CURVE_NODE_TYPE) {
			return null;
		}
		for (const child_id of liveNode.children) {
			const child = graphNodesById.get(child_id);
			if (!child || child.decl_id !== DECL_RANGE || child.node_type !== RANGE_NODE_TYPE) {
				continue;
			}
			return child;
		}
		return null;
	});

	let selected_key_node = $derived.by((): UiNodeDto | null => {
		if (selected_key_id === null) {
			return null;
		}
		return graphNodesById?.get(selected_key_id) ?? null;
	});

	let selected_curve_easing_node = $derived.by((): UiNodeDto | null => {
		if (!graphNodesById) {
			return null;
		}
		for (const owner_key_id of selected_curve_owner_key_ids) {
			const key_node = graphNodesById.get(owner_key_id);
			if (!key_node || key_node.node_type !== KEY_NODE_TYPE) {
				continue;
			}
			for (const child_id of key_node.children) {
				const child = graphNodesById.get(child_id);
				if (!child || child.decl_id !== DECL_EASING || child.node_type !== EASING_NODE_TYPE) {
					continue;
				}
				return child;
			}
		}
		return null;
	});

	$effect(() => {
		if (!graphNodesById || liveNode.node_type !== CURVE_NODE_TYPE) {
			selected_key_id = null;
			selected_key_ids = [];
			return;
		}

		const available_key_ids = new Set<NodeId>();
		for (const child_id of liveNode.children) {
			const child = graphNodesById.get(child_id);
			if (child?.node_type === KEY_NODE_TYPE) {
				available_key_ids.add(child.node_id);
			}
		}

		const filtered_selection = selected_key_ids.filter((key_id) => available_key_ids.has(key_id));
		if (!same_node_id_array(filtered_selection, selected_key_ids)) {
			selected_key_ids = filtered_selection;
		}
		if (selected_key_id !== null && !available_key_ids.has(selected_key_id)) {
			selected_key_id = filtered_selection[0] ?? null;
		}
		if (selected_key_id === null && filtered_selection.length > 0) {
			selected_key_id = filtered_selection[0];
		}
	});
</script>

{#snippet curveHeaderExtra()}
	<span class="curve-key-pill">{key_count_label}</span>
	{#if level > 0}
		<SelectNodeButton {node} />
	{/if}
{/snippet}

{#snippet curveContent()}
	<div class="animation-curve-node-inspector">
		<div class="curve-editor">
			<AnimationCurveNodeEditor
				curveNode={liveNode}
				bind:selected_key_id
				bind:selected_key_ids
				bind:selected_curve_owner_key_ids
				showGrid={true}
				showNumbers={true}
				showBounds={true}
				canvasHeight="min(24rem, 36vh)" />
		</div>
		<div class="curve-params-editor">
			{#if selected_key_node}
				<div class="selected-key-node-inspector">
					<NodeInspector nodes={[selected_key_node]} level={level + 1} order="solo" />
				</div>
			{:else if selected_curve_easing_node}
				<div class="selected-curve-node-inspector">
					<NodeInspector nodes={[selected_curve_easing_node]} level={level + 1} order="solo" />
				</div>
			{:else if editable_curve_range_node}
				<div class="range-node-inspector">
					<NodeInspector nodes={[editable_curve_range_node]} level={level + 1} order="solo" />
				</div>
			{:else}
				<div class="empty-state">Select a key or a curve segment to edit it.</div>
			{/if}
		</div>
	</div>
{/snippet}

{#if liveNode.node_type === CURVE_NODE_TYPE}
	{@render defaultHeader?.(curveHeaderExtra)}
	{@render defaultContent?.(curveContent)}
{/if}

<style>
	.animation-curve-node-inspector {
		display: flex;
		flex-direction: column;
		gap: 0.6rem;
		height: 100%;
		width: 100%;
		max-width: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		box-sizing: border-box;
		overflow: hidden;
	}

	.curve-editor {
		flex: 0 0 auto;
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		box-sizing: border-box;
		overflow: hidden;
	}

	.curve-editor :global(.curve-canvas-container),
	.curve-editor :global(.curve-canvas-wrap) {
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		overflow: hidden;
	}

	.curve-params-editor {
		display: flex;
		flex-direction: column;
		flex: 1 1 auto;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: auto;
	}

	.range-node-inspector {
		overflow: auto;
		min-inline-size: 0;
	}

	.selected-key-node-inspector {
		overflow: auto;
		height: 100%;
		min-inline-size: 0;
	}

	.selected-curve-node-inspector {
		overflow: auto;
		height: 100%;
		min-inline-size: 0;
	}

	.empty-state {
		font-size: 0.67rem;
		opacity: 0.68;
		padding: 0.45rem;
		border-radius: 0.28rem;
		background: rgba(255, 255, 255, 0.03);
	}

	.curve-key-pill {
		display: inline-flex;
		align-items: center;
		height: 1.02rem;
		padding: 0 0.42rem;
		border-radius: 999rem;
		font-size: 0.6rem;
		background: rgba(255, 225, 119, 0.15);
		border: solid 0.06rem rgba(255, 224, 106, 0.34);
	}
</style>
