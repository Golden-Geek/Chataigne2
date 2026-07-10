<script lang="ts">
	import { appState } from '../../../store/workbench.svelte';
	import EnableButton from '../../common/EnableButton.svelte';
	import type { UiNodeDto } from '../../../types';
	import Vec2PadEditor from '../inspector/parameters/Vec2PadEditor.svelte';
	import {
		getEffectiveWidgetVectorRange,
		getEnabledWidgetNumberOption
	} from './dashboard-node-widget-options';

	let {
		targetNode,
		widgetNode = null,
		insideLabel = null,
		showEnableButton = true
	} = $props<{
		targetNode: UiNodeDto;
		widgetNode?: UiNodeDto | null;
		insideLabel?: string | null;
		showEnableButton?: boolean;
		includeChildren?: boolean;
		editMode?: boolean;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveWidgetNode = $derived(
		widgetNode ? (graph?.nodesById.get(widgetNode.node_id) ?? widgetNode) : null
	);
	let liveTargetNode = $derived(graph?.nodesById.get(targetNode.node_id) ?? targetNode);
	let effectiveRange = $derived(
		getEffectiveWidgetVectorRange(graph, liveWidgetNode, liveTargetNode, 2)
	);
	let trailTime = $derived(getEnabledWidgetNumberOption(graph, liveWidgetNode, 'trail_time'));
	let showsEnableButton = $derived(showEnableButton && liveTargetNode.meta.can_be_disabled);
</script>

<div class="dashboard-node-widget-vec2-pad">
	{#if showsEnableButton}
		<div class="dashboard-node-widget-enable">
			<EnableButton node={liveTargetNode} />
		</div>
	{/if}
	{#if liveTargetNode.data.kind === 'parameter' && liveTargetNode.data.param.value.kind === 'vec2'}
		{#if typeof insideLabel === 'string' && insideLabel.trim().length > 0}
			<div class="dashboard-node-widget-vec2-pad-label">{insideLabel.trim()}</div>
		{/if}
		<Vec2PadEditor
			node={liveTargetNode}
			layoutMode="widget"
			rangeOverride={effectiveRange}
			trailSeconds={trailTime} />
	{:else}
		<div class="dashboard-node-widget-mode-empty">2D Pad mode only applies to vec2 parameters.</div>
	{/if}
</div>

<style>
	.dashboard-node-widget-vec2-pad {
		display: flex;
		flex: 1 1 auto;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		position: relative;
	}

	.dashboard-node-widget-enable {
		position: absolute;
		inset-block-start: 0.75rem;
		inset-inline-end: 0.75rem;
		z-index: 1;
		display: flex;
		align-items: center;
	}

	.dashboard-node-widget-vec2-pad-label {
		position: absolute;
		inset-block-start: 0.75rem;
		inset-inline-start: 0.75rem;
		z-index: 1;
		max-inline-size: calc(100% - 1.5rem);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		pointer-events: none;
	}

	.dashboard-node-widget-mode-empty {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: center;
		padding: 1rem;
		text-align: center;
	}
</style>
