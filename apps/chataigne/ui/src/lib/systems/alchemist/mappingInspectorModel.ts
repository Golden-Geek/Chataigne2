import type { UiCreatableUserItem, UiEditIntent, UiNodeDto } from 'golden_ui';
import type {
	ContextKeyDto,
	FormulaPreviewDemandDto,
	FormulaPreviewModeDto,
	ManagedRegionDefinitionDto,
	ManagedItemDto,
	MappingOutputSourceDto,
	MappingValueShapeDto,
	ProcessorUiDto
} from '../state_machine/generated';

export const isTupleMappingSurface = (processor: ProcessorUiDto | null): boolean => {
	if (!processor) return false;
	if (!processor.standard_mapping) return false;
	const regions = processor.managed_regions;
	return (
		regions.some((region) => region.kind === 'input_set') &&
		regions.some(
			(region) => region.kind === 'filter_pipeline' && region.filter_value_mode === 'tuple'
		) &&
		regions.some((region) => region.kind === 'output_set')
	);
};

export const mappingRegionNode = (
	processorNode: UiNodeDto,
	region: ManagedRegionDefinitionDto,
	nodesById: ReadonlyMap<number, UiNodeDto>
): UiNodeDto | null => {
	const root = directChild(processorNode, 'managed_regions', nodesById);
	return root ? directChild(root, `managed_region/${region.id}`, nodesById) : null;
};

export const directChild = (
	parent: UiNodeDto,
	declId: string,
	nodesById: ReadonlyMap<number, UiNodeDto>
): UiNodeDto | null => {
	for (const id of parent.children) {
		const child = nodesById.get(id);
		if (child?.decl_id === declId) return child;
	}
	return null;
};

export const shapeLabel = (shape: MappingValueShapeDto | null): string => {
	if (!shape) return 'Pending type check';
	if (shape.kind === 'incomplete' && shape.elements.length === 0) return 'No input';
	const types = shape.elements.map((element) => element.value_type ?? '?');
	if (shape.kind === 'tuple') return `Tuple (${types.join(', ')})`;
	if (shape.kind === 'compound') return `Compound ${types[0] ?? '?'}`;
	return types[0] ?? 'Unresolved';
};

export interface MappingSourceChoice {
	key: string;
	label: string;
	source: MappingOutputSourceDto;
}

export const mappingSourceChoices = (shape: MappingValueShapeDto): MappingSourceChoice[] => {
	const tuple = shape.elements.length > 1;
	const choices: MappingSourceChoice[] = tuple
		? []
		: [{ key: 'whole', label: 'Whole result', source: { kind: 'whole' } }];
	for (const element of shape.elements) {
		if (tuple) {
			choices.push({
				key: `element:${element.id}`,
				label: `${element.label} · ${element.value_type ?? '?'}`,
				source: { kind: 'element', id: element.id }
			});
		}
		for (const component of element.components) {
			choices.push({
				key: `component:${tuple ? element.id : ''}:${component}`,
				label: `${element.label}.${component}`,
				source: { kind: 'component', element: tuple ? element.id : null, component }
			});
		}
	}
	return choices;
};

export const sourceChoiceKey = (source: MappingOutputSourceDto): string => {
	switch (source.kind) {
		case 'whole':
			return 'whole';
		case 'element':
			return `element:${source.id}`;
		case 'component':
			return `component:${source.element ?? ''}:${source.component}`;
		case 'constant':
			return 'constant';
	}
};

export interface MappingWindow {
	start: number;
	end: number;
}

export const mappingWindow = (
	count: number,
	scrollTop: number,
	viewportHeight: number,
	rowHeight: number,
	overscan = 4
): MappingWindow => {
	if (count <= 48) return { start: 0, end: count };
	const safeHeight = Math.max(rowHeight, viewportHeight);
	const start = Math.max(0, Math.floor(Math.max(0, scrollTop) / rowHeight) - overscan);
	const end = Math.min(
		count,
		Math.ceil((Math.max(0, scrollTop) + safeHeight) / rowHeight) + overscan
	);
	return { start, end };
};

export const mappingNextIndex = (count: number, index: number, direction: -1 | 1): number =>
	Math.max(0, Math.min(count - 1, index + direction));

export const mappingCreateIntent = (
	region: UiNodeDto,
	item: UiCreatableUserItem
): UiEditIntent => ({
	kind: 'createUserItem',
	parent: region.node_id,
	node_type: item.node_type,
	initial_params: item.initial_params
});

export const mappingMoveIntent = (
	region: UiNodeDto,
	item: UiNodeDto,
	direction: -1 | 1
): UiEditIntent | null => {
	const index = region.children.indexOf(item.node_id);
	const siblingId = region.children[index + (direction < 0 ? -2 : 1)];
	if (index < 0 || (direction < 0 && index === 0) || (direction > 0 && siblingId === undefined)) {
		return null;
	}
	return {
		kind: 'moveNode',
		node: item.node_id,
		new_parent: region.node_id,
		...(siblingId === undefined ? {} : { new_prev_sibling: siblingId })
	};
};

export const mappingDuplicateIntent = (region: UiNodeDto, item: UiNodeDto): UiEditIntent => ({
	kind: 'duplicateNode',
	source: item.node_id,
	new_parent: region.node_id,
	new_prev_sibling: item.node_id
});

export const mappingPreviewMode = (
	processorId: string,
	item: ManagedItemDto | null,
	contextKey: ContextKeyDto | null
): FormulaPreviewModeDto =>
	item
		? {
				kind: 'processor_selected_stages',
				processor_id: processorId,
				context_key: contextKey,
				node_ids: [item.anode_id]
			}
		: { kind: 'processor_inspection', processor_id: processorId };

export const mappingPreviewDemand = (
	subscriptionId: string,
	mode: FormulaPreviewModeDto | null
): FormulaPreviewDemandDto => ({ subscription_id: subscriptionId, mode });
