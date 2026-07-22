import type { UiNodeDto } from '../../../types';
import DashboardNodeWidgetInspectorContent from './DashboardNodeWidgetInspectorContent.svelte';
import DashboardNodeWidgetNumberRotaryContent from './DashboardNodeWidgetNumberRotaryContent.svelte';
import DashboardNodeWidgetNumberSliderContent from './DashboardNodeWidgetNumberSliderContent.svelte';
import DashboardNodeWidgetParameterEditorContent from './DashboardNodeWidgetParameterEditorContent.svelte';
import DashboardNodeWidgetVec2PadContent from './DashboardNodeWidgetVec2PadContent.svelte';

export interface DashboardNodeWidgetTypeEntry {
	id: string;
	component: any;
	usesLabelPlacement: boolean;
	isCompatible?: (targetNode: UiNodeDto) => boolean;
}

const builtinWidgetTypes = new Map<string, DashboardNodeWidgetTypeEntry>([
	[
		'default',
		{
			id: 'default',
			component: DashboardNodeWidgetParameterEditorContent,
			usesLabelPlacement: true,
			isCompatible: (targetNode) => targetNode.data.kind === 'parameter'
		}
	],
	[
		'inspector',
		{
			id: 'inspector',
			component: DashboardNodeWidgetInspectorContent,
			usesLabelPlacement: true,
			isCompatible: () => true
		}
	],
	[
		'slider',
		{
			id: 'slider',
			component: DashboardNodeWidgetNumberSliderContent,
			usesLabelPlacement: true,
			isCompatible: (targetNode) =>
				targetNode.data.kind === 'parameter' &&
				(targetNode.data.param.value.kind === 'int' || targetNode.data.param.value.kind === 'float')
		}
	],
	[
		'rotary',
		{
			id: 'rotary',
			component: DashboardNodeWidgetNumberRotaryContent,
			usesLabelPlacement: true,
			isCompatible: (targetNode) =>
				targetNode.data.kind === 'parameter' &&
				(targetNode.data.param.value.kind === 'int' || targetNode.data.param.value.kind === 'float')
		}
	],
	[
		'vec2Pad',
		{
			id: 'vec2Pad',
			component: DashboardNodeWidgetVec2PadContent,
			usesLabelPlacement: true,
			isCompatible: (targetNode) =>
				targetNode.data.kind === 'parameter' && targetNode.data.param.value.kind === 'vec2'
		}
	]
]);

const customWidgetTypes = new Map<string, DashboardNodeWidgetTypeEntry>();

const getWidgetTypeEntry = (typeId: string): DashboardNodeWidgetTypeEntry | null =>
	customWidgetTypes.get(typeId) ?? builtinWidgetTypes.get(typeId) ?? null;

export const registerDashboardNodeWidgetType = (entry: DashboardNodeWidgetTypeEntry): void => {
	const typeId = entry.id.trim();
	if (typeId.length === 0) {
		throw new Error('Dashboard node widget type registration requires a non-empty id.');
	}
	customWidgetTypes.set(typeId, {
		...entry,
		id: typeId
	});
};

export const unregisterDashboardNodeWidgetType = (typeId: string): void => {
	customWidgetTypes.delete(typeId.trim());
};

export const clearDashboardNodeWidgetTypes = (): void => {
	customWidgetTypes.clear();
};

export const resolveDashboardNodeWidgetType = (
	targetNode: UiNodeDto,
	requestedTypeId: string
): DashboardNodeWidgetTypeEntry | null => {
	const normalizedRequestedTypeId = requestedTypeId.trim();
	const requestedWidgetType =
		normalizedRequestedTypeId.length > 0 ? getWidgetTypeEntry(normalizedRequestedTypeId) : null;
	if (requestedWidgetType && (requestedWidgetType.isCompatible?.(targetNode) ?? true)) {
		return requestedWidgetType;
	}

	for (const widgetType of [...customWidgetTypes.values(), ...builtinWidgetTypes.values()]) {
		if (widgetType.isCompatible?.(targetNode) ?? true) {
			return widgetType;
		}
	}

	return null;
};
