<script lang="ts">
	import { appState } from '../../../store/workbench.svelte';
	import EnableButton from '../../common/EnableButton.svelte';
	import type { UiNodeDto } from '../../../types';
	import NumberEditor from '../inspector/parameters/NumberEditor.svelte';
	import {
		clampWidgetMaxDecimals,
		getEffectiveWidgetScalarRange,
		getWidgetBoolOption,
		getWidgetIntOption
	} from './dashboard-node-widget-options';

	type NumberEditorOptions = {
		show_value_field?: boolean;
		max_decimals?: number;
		inside_label?: string;
	};

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
	let paramKind = $derived(
		liveTargetNode.data.kind === 'parameter' ? liveTargetNode.data.param.value.kind : null
	);
	let insideLabelText = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsEnableButton = $derived(showEnableButton && liveTargetNode.meta.can_be_disabled);
	let sliderPresentation = $derived.by(
		(): NumberEditorOptions => ({
			show_value_field: getWidgetBoolOption(graph, liveWidgetNode, 'slider_show_value_field', true),
			max_decimals: clampWidgetMaxDecimals(
				getWidgetIntOption(graph, liveWidgetNode, 'slider_max_decimals', 3),
				3
			),
			inside_label: insideLabelText.length > 0 ? insideLabelText : undefined
		})
	);
	let effectiveRange = $derived(
		getEffectiveWidgetScalarRange(graph, liveWidgetNode, liveTargetNode)
	);
</script>

<div class="dashboard-node-widget-number-slider">
	{#if showsEnableButton}
		<div class="dashboard-node-widget-enable">
			<EnableButton node={liveTargetNode} />
		</div>
	{/if}
	{#if paramKind === 'int' || paramKind === 'float'}
		<div class="dashboard-node-widget-number-slider-body">
			<NumberEditor
				node={liveTargetNode}
				layoutMode="widget"
				presentation={sliderPresentation}
				rangeOverride={effectiveRange} />
		</div>
	{:else}
		<div class="dashboard-node-widget-mode-empty">
			Slider mode only applies to int and float parameters.
		</div>
	{/if}
</div>

<style>
	.dashboard-node-widget-number-slider,
	.dashboard-node-widget-number-slider-body {
		display: flex;
		flex: 1 1 auto;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-number-slider {
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

	.dashboard-node-widget-number-slider :global(.number-property-container),
	.dashboard-node-widget-number-slider :global(.slider-wrapper) {
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-number-slider :global(.slider-wrapper) {
		flex: 1 1 auto;
	}

	.dashboard-node-widget-mode-empty {
		display: flex;
		flex: 1 1 auto;
		align-items: center;
		justify-content: center;
		padding: 0.75rem;
		text-align: center;
		font-size: 0.72rem;
		opacity: 0.72;
	}
</style>
