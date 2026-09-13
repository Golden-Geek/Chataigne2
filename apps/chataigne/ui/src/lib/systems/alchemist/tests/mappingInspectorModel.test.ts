import { describe, expect, it } from 'vitest';
import type { UiCreatableUserItem, UiNodeDto } from 'golden_ui';
import type {
	ManagedItemDto,
	MappingValueShapeDto,
	ProcessorUiDto
} from '../../state_machine/generated';
import {
	isTupleMappingSurface,
	mappingCreateIntent,
	mappingDuplicateIntent,
	mappingMoveIntent,
	mappingNextIndex,
	mappingPreviewDemand,
	mappingPreviewMode,
	mappingRegionNode,
	mappingSourceChoices,
	mappingWindow,
	shapeLabel,
	sourceChoiceKey
} from '../mappingInspectorModel';

const node = (id: number, declId = '', children: number[] = []): UiNodeDto =>
	({ node_id: id, uuid: `node-${id}`, decl_id: declId, children }) as UiNodeDto;

const element = (id: string, valueType: string, components: Array<'x' | 'y' | 'z'> = []) => ({
	id,
	label: id.toUpperCase(),
	value_type: valueType,
	minimum: null,
	maximum: null,
	unit: null,
	components
});

const shape = (
	kind: MappingValueShapeDto['kind'],
	elements: MappingValueShapeDto['elements']
): MappingValueShapeDto => ({ kind, elements });

const managedItem = { id: 'item-1', anode_id: 'anode-1' } as ManagedItemDto;

describe('Mapping product inspector model', () => {
	it('recognizes only the backend-declared ordered tuple Mapping surface', () => {
		const regions = [
			{ id: 'inputs', kind: 'input_set', filter_value_mode: 'routed' },
			{ id: 'filters', kind: 'filter_pipeline', filter_value_mode: 'tuple' },
			{ id: 'outputs', kind: 'output_set', filter_value_mode: 'routed' }
		] as ProcessorUiDto['managed_regions'];
		const processor = { managed_regions: regions, standard_mapping: true } as ProcessorUiDto;
		expect(isTupleMappingSurface(processor)).toBe(true);
		expect(isTupleMappingSurface({ ...processor, standard_mapping: false })).toBe(false);
		expect(
			isTupleMappingSurface({
				...processor,
				managed_regions: regions.filter((region) => region.kind !== 'output_set')
			})
		).toBe(false);
		expect(
			isTupleMappingSurface({
				...processor,
				managed_regions: regions.map((region) =>
					region.kind === 'filter_pipeline' ? { ...region, filter_value_mode: 'routed' } : region
				)
			})
		).toBe(false);
	});

	it('follows the actual processor managed-region node path', () => {
		const processor = node(1, '', [2]);
		const regionsRoot = node(2, 'managed_regions', [3]);
		const filters = node(3, 'managed_region/filters');
		const nodes = new Map([processor, regionsRoot, filters].map((entry) => [entry.node_id, entry]));
		expect(
			mappingRegionNode(
				processor,
				{ id: 'filters' } as ProcessorUiDto['managed_regions'][number],
				nodes
			)
		).toBe(filters);
	});

	it('keeps X/Y/Z as independently addressable tuple elements through repeated shape changes', () => {
		const xyz = shape('tuple', [
			element('x', 'float'),
			element('y', 'float'),
			element('z', 'float')
		]);
		expect(shapeLabel(xyz)).toBe('Tuple (float, float, float)');
		expect(mappingSourceChoices(xyz).map((choice) => choice.key)).toEqual([
			'element:x',
			'element:y',
			'element:z'
		]);
		const reordered = shape('tuple', [xyz.elements[2], xyz.elements[0], xyz.elements[1]]);
		expect(mappingSourceChoices(reordered).map((choice) => choice.key)).toEqual([
			'element:z',
			'element:x',
			'element:y'
		]);
		const packed = shape('compound', [element('packed', 'vec3', ['x', 'y', 'z'])]);
		expect(shapeLabel(packed)).toBe('Compound vec3');
		expect(mappingSourceChoices(packed).map((choice) => choice.key)).toEqual([
			'whole',
			'component::x',
			'component::y',
			'component::z'
		]);
		expect(sourceChoiceKey({ kind: 'component', element: null, component: 'z' })).toBe(
			'component::z'
		);
	});

	it('uses backend item specs for creation and stable node IDs for reorder and duplicate', () => {
		const region = node(10, '', [11, 12, 13]);
		const first = node(11);
		const middle = node(12);
		const last = node(13);
		const item = {
			node_type: 'alchemist.filter/sum',
			initial_params: []
		} as unknown as UiCreatableUserItem;
		expect(mappingCreateIntent(region, item)).toEqual({
			kind: 'createUserItem',
			parent: 10,
			node_type: item.node_type,
			initial_params: []
		});
		expect(mappingMoveIntent(region, middle, -1)).toEqual({
			kind: 'moveNode',
			node: 12,
			new_parent: 10
		});
		expect(mappingMoveIntent(region, middle, 1)).toEqual({
			kind: 'moveNode',
			node: 12,
			new_parent: 10,
			new_prev_sibling: 13
		});
		expect(mappingMoveIntent(region, first, -1)).toBeNull();
		expect(mappingMoveIntent(region, last, 1)).toBeNull();
		expect(mappingDuplicateIntent(region, middle)).toEqual({
			kind: 'duplicateNode',
			source: 12,
			new_parent: 10,
			new_prev_sibling: 12
		});
	});

	it('keeps keyboard movement and ten-thousand-row windows bounded', () => {
		expect(mappingNextIndex(3, 0, -1)).toBe(0);
		expect(mappingNextIndex(3, 1, 1)).toBe(2);
		expect(mappingNextIndex(3, 2, 1)).toBe(2);
		expect(mappingWindow(10_000, 0, 260, 42.4)).toEqual({ start: 0, end: 11 });
		const tail = mappingWindow(10_000, 9999 * 42.4, 260, 42.4);
		expect(tail.end).toBe(10_000);
		expect(tail.end - tail.start).toBeLessThan(20);
	});

	it('requests no samples until a stage is selected and releases its lease on teardown', () => {
		const context = {
			parts: [{ axis_id: 'device', axis_label: 'Device', item_id: 'b', item_label: 'B', index: 1 }]
		};
		expect(mappingPreviewMode('processor', null, null)).toEqual({
			kind: 'processor_inspection',
			processor_id: 'processor'
		});
		const selected = mappingPreviewMode('processor', managedItem, context);
		expect(selected).toEqual({
			kind: 'processor_selected_stages',
			processor_id: 'processor',
			context_key: context,
			node_ids: ['anode-1']
		});
		expect(mappingPreviewMode('processor', managedItem, null)).toEqual({
			...selected,
			context_key: null
		});
		expect(mappingPreviewDemand('lease', null)).toEqual({ subscription_id: 'lease', mode: null });
	});
});
