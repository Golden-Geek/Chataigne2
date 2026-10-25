import { sendSetParamIntent } from '../../../../store/ui-intents';
import { type ParamValue } from '../../../../types';
import { parseCssValue } from '../../../../css-value';
import { resolveDashboardNodeWidgetType } from '../dashboard-node-widget-registry';
import { formatParamValue, getDirectParam, resolveReferenceTarget } from '../dashboard-model';
import type { GraphState } from '../../../../store/graph.svelte';
import type { UiNodeDto } from '../../../../types';
import type { DashboardPlacement } from '../dashboard-model';
interface WidgetContentContext {
	readonly graph: GraphState | null;
	readonly node: UiNodeDto;
	readonly isNodeWidget: boolean;
	readonly editMode: boolean;
	readonly placement: DashboardPlacement;
}
export function createDashboardWidgetContent(context: WidgetContentContext) {
	let graph = $derived(context.graph);
	let liveNode = $derived(context.node);
	let isNodeWidget = $derived(context.isNodeWidget);
	let editMode = $derived(context.editMode);
	let widgetLabelPlacement = $derived(context.placement.labelPlacement);
	let widgetLabelText = $derived(liveNode.meta.label.trim());
	const widgetKind = $derived.by(() => {
		const widgetKindParam = getDirectParam(graph, liveNode, 'widget_kind');
		return widgetKindParam?.value.kind === 'enum' ? widgetKindParam.value.value : 'text';
	});

	let textParam = $derived(getDirectParam(graph, liveNode, 'text'));

	let placeholderParam = $derived(getDirectParam(graph, liveNode, 'placeholder'));

	let valueRangeParam = $derived(getDirectParam(graph, liveNode, 'value_range'));

	let stepParam = $derived(getDirectParam(graph, liveNode, 'step'));

	let multilineParam = $derived(getDirectParam(graph, liveNode, 'multiline'));

	let defaultCheckedParam = $derived(getDirectParam(graph, liveNode, 'default_checked'));

	let targetNodeParam = $derived(getDirectParam(graph, liveNode, 'target_node'));

	let targetParamParam = $derived(getDirectParam(graph, liveNode, 'target_param'));

	let widgetTypeParam = $derived(getDirectParam(graph, liveNode, 'widget_type'));

	let maxChildLevelParam = $derived(getDirectParam(graph, liveNode, 'max_child_level'));

	let showEnableButtonParam = $derived(getDirectParam(graph, liveNode, 'show_enable_button'));

	let boundNode = $derived(resolveReferenceTarget(graph, targetNodeParam?.value ?? null));

	let boundParamNode = $derived(resolveReferenceTarget(graph, targetParamParam?.value ?? null));

	let boundParam = $derived(
		boundParamNode?.data.kind === 'parameter' ? boundParamNode.data.param : null
	);

	const textConfig = $derived(textParam?.value.kind === 'str' ? textParam.value.value : '');

	const placeholderConfig = $derived(
		placeholderParam?.value.kind === 'str' ? placeholderParam.value.value : ''
	);

	const valueRange = $derived(
		valueRangeParam?.value.kind === 'vec2' ? valueRangeParam.value.value : [0, 1]
	);

	const sliderStep = $derived(stepParam?.value.kind === 'float' ? stepParam.value.value : 0.01);

	const multiline = $derived(
		multilineParam?.value.kind === 'bool' ? multilineParam.value.value : false
	);

	const defaultChecked = $derived(
		defaultCheckedParam?.value.kind === 'bool' ? defaultCheckedParam.value.value : false
	);

	const requestedWidgetType = $derived.by(() => {
		if (widgetTypeParam?.value.kind === 'enum' || widgetTypeParam?.value.kind === 'str') {
			return widgetTypeParam.value.value;
		}
		return 'auto';
	});

	const maxChildLevel = $derived.by(() => {
		if (maxChildLevelParam?.value.kind === 'int' || maxChildLevelParam?.value.kind === 'float') {
			return Math.max(0, Math.round(maxChildLevelParam.value.value));
		}
		return 2;
	});

	const showEnableButton = $derived(
		showEnableButtonParam?.value.kind === 'bool' ? showEnableButtonParam.value.value : true
	);

	const resolvedNodeWidgetType = $derived.by(() =>
		boundNode ? resolveDashboardNodeWidgetType(boundNode, requestedWidgetType) : null
	);

	const NodeWidgetDisplayComponent = $derived(resolvedNodeWidgetType?.component ?? null);

	const widgetUsesLabelPlacement = $derived.by(() => {
		if (isNodeWidget) {
			return resolvedNodeWidgetType?.usesLabelPlacement ?? false;
		}
		return true;
	});

	const widgetWrapperLabelPlacement = $derived.by(() => {
		if (
			!widgetUsesLabelPlacement ||
			widgetLabelPlacement === null ||
			widgetLabelPlacement === 'inside'
		) {
			return null;
		}
		return widgetLabelPlacement;
	});

	const hasExternalWidgetLabel = $derived(
		widgetWrapperLabelPlacement !== null && widgetLabelText.length > 0
	);

	const hasInsideWidgetLabel = $derived(
		widgetUsesLabelPlacement && widgetLabelPlacement === 'inside' && widgetLabelText.length > 0
	);

	const genericDisplayValue = $derived.by(() => {
		if (widgetKind === 'text' && boundParam) {
			return formatParamValue(boundParam.value);
		}
		return textConfig;
	});

	const genericSliderValue = $derived.by(() => {
		if (boundParam?.value.kind === 'float' || boundParam?.value.kind === 'int') {
			return boundParam.value.value;
		}
		return valueRange[0];
	});

	const genericSliderDisabled = $derived(
		editMode ||
			!boundParam ||
			(boundParam.value.kind !== 'float' && boundParam.value.kind !== 'int') ||
			boundParamNode?.meta.enabled === false
	);

	const applyGenericParamValue = async (value: ParamValue): Promise<void> => {
		if (!boundParamNode || boundParamNode.data.kind !== 'parameter') {
			return;
		}
		await sendSetParamIntent(
			boundParamNode.node_id,
			value,
			boundParamNode.data.param.event_behaviour
		);
	};

	const triggerGenericAction = async (): Promise<void> => {
		if (boundParam?.value.kind === 'trigger') {
			await applyGenericParamValue({ kind: 'trigger' });
			return;
		}
		if (boundParam?.value.kind === 'bool') {
			await applyGenericParamValue({
				kind: 'bool',
				value: !boundParam.value.value
			});
		}
	};

	const applySliderValue = async (raw: string): Promise<void> => {
		if (!boundParam) {
			return;
		}
		const numericValue = Number(raw);
		if (!Number.isFinite(numericValue)) {
			return;
		}
		if (boundParam.value.kind === 'int') {
			await applyGenericParamValue({ kind: 'int', value: Math.round(numericValue) });
			return;
		}
		if (boundParam.value.kind === 'float') {
			await applyGenericParamValue({ kind: 'float', value: numericValue });
		}
	};

	const applyStringValue = async (raw: string): Promise<void> => {
		if (!boundParam) {
			return;
		}
		if (boundParam.value.kind === 'css_value') {
			const parsed = parseCssValue(raw, boundParam.value.unit);
			if (!parsed) {
				return;
			}
			await applyGenericParamValue({ kind: 'css_value', value: parsed.value, unit: parsed.unit });
			return;
		}
		if (
			boundParam.value.kind === 'str' ||
			boundParam.value.kind === 'file' ||
			boundParam.value.kind === 'enum'
		) {
			await applyGenericParamValue({ kind: boundParam.value.kind, value: raw });
		}
	};
	return {
		get boundNode() {
			return boundNode;
		},
		get boundParamNode() {
			return boundParamNode;
		},
		get boundParam() {
			return boundParam;
		},
		get valueRange() {
			return valueRange;
		},
		get sliderStep() {
			return sliderStep;
		},
		get defaultChecked() {
			return defaultChecked;
		},
		get maxChildLevel() {
			return maxChildLevel;
		},
		get showEnableButton() {
			return showEnableButton;
		},
		get NodeWidgetDisplayComponent() {
			return NodeWidgetDisplayComponent;
		},
		get widgetWrapperLabelPlacement() {
			return widgetWrapperLabelPlacement;
		},
		get hasExternalWidgetLabel() {
			return hasExternalWidgetLabel;
		},
		get hasInsideWidgetLabel() {
			return hasInsideWidgetLabel;
		},
		get genericSliderValue() {
			return genericSliderValue;
		},
		get genericSliderDisabled() {
			return genericSliderDisabled;
		},
		applyGenericParamValue,
		triggerGenericAction,
		applySliderValue,
		applyStringValue,
		get widgetKind() {
			return widgetKind;
		},
		get textConfig() {
			return textConfig;
		},
		get placeholderConfig() {
			return placeholderConfig;
		},
		get multiline() {
			return multiline;
		},
		get genericDisplayValue() {
			return genericDisplayValue;
		}
	};
}
