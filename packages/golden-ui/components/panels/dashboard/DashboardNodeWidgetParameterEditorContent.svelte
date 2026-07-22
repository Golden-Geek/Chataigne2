<script lang="ts">
	import { appState } from '../../../store/workbench.svelte';
	import EnableButton from '../../common/EnableButton.svelte';
	import type { UiNodeDto } from '../../../types';
	import CheckboxEditor from '../inspector/parameters/CheckboxEditor.svelte';
	import ColorPickerEditor from '../inspector/parameters/ColorPickerEditor.svelte';
	import CssValueEditor from '../inspector/parameters/CssValueEditor.svelte';
	import DropdownEditor from '../inspector/parameters/DropdownEditor.svelte';
	import { propertiesInspectorClass } from '../inspector/inspector.svelte';
	import MultiNumberEditor from '../inspector/parameters/MultiNumberEditor.svelte';
	import NumberEditor from '../inspector/parameters/NumberEditor.svelte';
	import ReferenceEditor from '../inspector/parameters/ReferenceEditor.svelte';
	import TextInputEditor from '../inspector/parameters/TextInputEditor.svelte';
	import TriggerEditor from '../inspector/parameters/TriggerEditor.svelte';
	import {
		clampWidgetMaxDecimals,
		getEffectiveWidgetScalarRange,
		getEffectiveWidgetVectorRange,
		getWidgetBoolOption,
		getWidgetEnumOption,
		getWidgetIntOption
	} from './dashboard-node-widget-options';

	type NumberEditorOptions = {
		show_value_field?: boolean;
		max_decimals?: number;
		inside_label?: string;
	};

	type VectorEditorOptions = {
		layout?: 'inline' | 'column';
		show_value_fields?: boolean;
		max_decimals?: number;
	};

	type ColorEditorOptions = {
		force_expanded?: boolean;
		show_hex?: boolean;
		show_rgba_fields?: boolean;
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
	let valueEditorReadOnly = $derived(
		liveTargetNode.data.kind === 'parameter' &&
			liveTargetNode.data.param.control.mode !== 'manual' &&
			liveTargetNode.data.param.control.mode !== 'templateText'
	);

	let numberPresentation = $derived.by(
		(): NumberEditorOptions => ({
			show_value_field: getWidgetBoolOption(graph, liveWidgetNode, 'slider_show_value_field', true),
			max_decimals: clampWidgetMaxDecimals(
				getWidgetIntOption(graph, liveWidgetNode, 'slider_max_decimals', 3),
				3
			)
		})
	);

	let vectorPresentation = $derived.by(
		(): VectorEditorOptions => ({
			layout: getWidgetEnumOption(graph, liveWidgetNode, 'vector_layout', 'inline', [
				'inline',
				'column'
			] as const),
			show_value_fields: getWidgetBoolOption(
				graph,
				liveWidgetNode,
				'vector_show_value_fields',
				true
			),
			max_decimals: clampWidgetMaxDecimals(
				getWidgetIntOption(graph, liveWidgetNode, 'vector_max_decimals', 2),
				2
			)
		})
	);

	let colorPresentation = $derived.by(
		(): ColorEditorOptions => ({
			force_expanded: getWidgetBoolOption(graph, liveWidgetNode, 'color_force_expanded', false),
			show_hex: getWidgetBoolOption(graph, liveWidgetNode, 'color_show_hex', true),
			show_rgba_fields: getWidgetBoolOption(graph, liveWidgetNode, 'color_show_rgba_fields', true)
		})
	);
	let insideLabelText = $derived(typeof insideLabel === 'string' ? insideLabel.trim() : '');
	let showsInsideLabel = $derived(insideLabelText.length > 0);
	let editorInsideLabel = $derived(showsInsideLabel ? insideLabelText : null);
	let showsEnableButton = $derived(showEnableButton && liveTargetNode.meta.can_be_disabled);
	let effectiveNumberRange = $derived(
		getEffectiveWidgetScalarRange(graph, liveWidgetNode, liveTargetNode)
	);
	let effectiveVectorRange = $derived.by(() => {
		if (paramKind === 'vec2') {
			return getEffectiveWidgetVectorRange(graph, liveWidgetNode, liveTargetNode, 2);
		}
		if (paramKind === 'vec3') {
			return getEffectiveWidgetVectorRange(graph, liveWidgetNode, liveTargetNode, 3);
		}
		return null;
	});

	let editorEntry = $derived.by(() => {
		if (liveTargetNode.data.kind !== 'parameter') {
			return null;
		}
		return propertiesInspectorClass[liveTargetNode.data.param.value.kind] ?? null;
	});

	let EditorComponent = $derived(editorEntry?.component ?? null);
</script>

<div class="dashboard-node-widget-parameter-editor">
	{#if showsEnableButton}
		<div class="dashboard-node-widget-enable">
			<EnableButton node={liveTargetNode} />
		</div>
	{/if}
	{#if targetNode.data.kind !== 'parameter'}
		<div class="dashboard-node-widget-mode-empty">
			Default widget type only applies to parameters.
		</div>
	{:else if paramKind === 'int' || paramKind === 'float'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<NumberEditor
				node={liveTargetNode}
				layoutMode="widget"
				readOnly={valueEditorReadOnly}
				presentation={{
					...numberPresentation,
					inside_label: editorInsideLabel ?? undefined
				}}
				rangeOverride={effectiveNumberRange} />
		</div>
	{:else if paramKind === 'vec2' || paramKind === 'vec3'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<MultiNumberEditor
				node={liveTargetNode}
				layoutMode="widget"
				readOnly={valueEditorReadOnly}
				presentation={vectorPresentation}
				rangeOverride={effectiveVectorRange} />
		</div>
	{:else if paramKind === 'color'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout color-layout">
			<ColorPickerEditor
				node={liveTargetNode}
				layoutMode="widget"
				readOnly={valueEditorReadOnly}
				presentation={colorPresentation} />
		</div>
	{:else if paramKind === 'bool'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout checkbox-layout">
			<CheckboxEditor
				node={liveTargetNode}
				layoutMode="widget"
				insideLabel={editorInsideLabel}
				readOnly={valueEditorReadOnly} />
		</div>
	{:else if paramKind === 'trigger'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<TriggerEditor
				node={liveTargetNode}
				layoutMode="widget"
				insideLabel={editorInsideLabel}
				readOnly={valueEditorReadOnly} />
		</div>
	{:else if paramKind === 'str'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<TextInputEditor
				node={liveTargetNode}
				layoutMode="widget"
				insideLabel={editorInsideLabel}
				readOnly={valueEditorReadOnly} />
		</div>
	{:else if paramKind === 'enum'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<DropdownEditor
				node={liveTargetNode}
				layoutMode="widget"
				insideLabel={editorInsideLabel}
				readOnly={valueEditorReadOnly} />
		</div>
	{:else if paramKind === 'reference'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<ReferenceEditor
				node={liveTargetNode}
				layoutMode="widget"
				insideLabel={editorInsideLabel}
				readOnly={valueEditorReadOnly} />
		</div>
	{:else if paramKind === 'css_value'}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<CssValueEditor node={liveTargetNode} layoutMode="widget" readOnly={valueEditorReadOnly} />
		</div>
	{:else if EditorComponent}
		<div class="dashboard-node-widget-parameter-editor-body widget-layout">
			<EditorComponent node={liveTargetNode} readOnly={valueEditorReadOnly} />
		</div>
	{:else}
		<div class="dashboard-node-widget-mode-empty">
			No editor is registered for this parameter type.
		</div>
	{/if}
</div>

<style>
	.dashboard-node-widget-parameter-editor {
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

	.dashboard-node-widget-parameter-editor-body {
		display: flex;
		flex: 1 1 auto;
		align-items: stretch;
		justify-content: stretch;
		inline-size: 100%;
		max-inline-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-parameter-editor-body.widget-layout {
		block-size: 100%;
		overflow: hidden;
	}

	.dashboard-node-widget-parameter-editor-body.color-layout,
	.dashboard-node-widget-parameter-editor-body.checkbox-layout {
		overflow: hidden;
	}

	.dashboard-node-widget-parameter-editor :global(.number-property-container),
	.dashboard-node-widget-parameter-editor :global(.slider-wrapper),
	.dashboard-node-widget-parameter-editor :global(.css-value-editor),
	.dashboard-node-widget-parameter-editor :global(.multi-number-editor),
	.dashboard-node-widget-parameter-editor :global(.color-picker-editor),
	.dashboard-node-widget-parameter-editor :global(.editor-checkbox-shell),
	.dashboard-node-widget-parameter-editor :global(.trigger),
	.dashboard-node-widget-parameter-editor :global(.reference-property-container),
	.dashboard-node-widget-parameter-editor :global(.text-input-editor),
	.dashboard-node-widget-parameter-editor :global(.dropdown-editor),
	.dashboard-node-widget-parameter-editor :global(.dropdown-container) {
		inline-size: 100%;
		block-size: 100%;
		max-inline-size: none;
		min-inline-size: 0;
		min-block-size: 0;
	}

	.dashboard-node-widget-parameter-editor :global(.slider-wrapper) {
		flex: 1 1 auto;
	}

	.dashboard-node-widget-parameter-editor :global(.number-property-container.widget-layout),
	.dashboard-node-widget-parameter-editor :global(.multi-number-editor.widget-layout) {
		height: 100%;
	}

	.dashboard-node-widget-parameter-editor :global(textarea),
	.dashboard-node-widget-parameter-editor :global(input[type='text']),
	.dashboard-node-widget-parameter-editor :global(input[type='number']),
	.dashboard-node-widget-parameter-editor :global(select) {
		max-inline-size: none;
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
