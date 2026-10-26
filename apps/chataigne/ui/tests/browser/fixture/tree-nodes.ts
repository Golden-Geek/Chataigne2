import { fixtureNode } from 'golden_ui/testing/session';
import type { UiNodeDto } from 'golden_ui';

export type AppTreeKind = 'modules' | 'formulas' | 'processors' | 'properties';
const kinds = {
	modules: ['module_manager', 'module_folder', 'fixture_module', 'module'],
	formulas: [
		'alchemist_formula_library',
		'alchemist_formula_folder',
		'alchemist_formula',
		'alchemist_formula'
	],
	processors: [
		'state_processor_manager',
		'state_processor_folder',
		'state_processor',
		'state_processor'
	],
	properties: [
		'alchemist_property_manager',
		'alchemist_property_folder',
		'alchemist_property',
		'alchemist_property'
	]
};

export function appTreeNodes(kind: AppTreeKind, size: number): UiNodeDto[] {
	const [managerType, folderType, leafType, itemKind] = kinds[kind];
	const nodes = [
		fixtureNode(0, 'Project', [1]),
		fixtureNode(
			1,
			'Items',
			Array.from({ length: size }, (_, index) => index + 2),
			managerType
		)
	];
	for (let index = 0; index < size; index++) {
		const id = index + 2;
		const entry = fixtureNode(
			id,
			`Item ${String(index).padStart(6, '0')}`,
			index === 0 ? [size + 2] : index === 1 ? [size + 4] : [],
			index === 0 ? folderType : leafType
		);
		entry.user_item_kind =
			index === 0 && kind === 'formulas' ? 'alchemist_formula_folder' : itemKind;
		entry.user_role = 'itemRoot';
		if (index === 0) entry.meta.presentation = { collapsed: true };
		if (kind === 'formulas' && index === 1)
			entry.meta.tags = ['chataigne.formula.external.builtin:fixture'];
		nodes.push(entry);
	}
	nodes.push(
		fixtureNode(size + 2, 'Nested folder', [size + 3], folderType),
		fixtureNode(size + 3, 'Nested leaf', [], leafType),
		fixtureNode(size + 4, 'Private implementation', [], leafType)
	);
	for (const entry of nodes.slice(-3)) {
		entry.user_item_kind =
			entry.node_type === 'alchemist_formula_folder' ? 'alchemist_formula_folder' : itemKind;
		entry.user_role = entry.meta.label === 'Private implementation' ? 'regular' : 'itemRoot';
	}
	if (kind === 'properties') {
		const formula = fixtureNode(size + 5, 'Property owner', [1], 'alchemist_formula');
		formula.user_item_kind = 'alchemist_formula';
		formula.user_role = 'itemRoot';
		nodes[0].children = [formula.node_id];
		nodes[1].decl_id = 'properties';
		nodes.push(formula);
	}
	return nodes;
}
