import { describe, expect, it } from 'vitest';
import type { NodeId, UiNodeDto } from 'golden_ui';

import {
	FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX,
	FORMULA_EXTERNAL_FILE_DECL_ID,
	FORMULA_EXTERNAL_FILE_TAG,
	FORMULA_EXTERNAL_READ_ONLY_TAG,
	FORMULA_LIBRARY_NODE_TYPE,
	FORMULA_LIBRARY_SHARED_DIR_DECL_ID,
	PREFERENCES_DATA_FOLDER_DECL_ID,
	PREFERENCES_DECL_ID,
	PREFERENCES_SAVE_AND_LOAD_DECL_ID,
	formulaExternalFilePath,
	formulaIsBuiltIn,
	formulaIsExternalFile,
	formulaIsReadOnly,
	formulaSourceDisplay,
	formulaSourceKind,
	sharedFormulaDir,
	type FormulaSourceGraph
} from '../formulaSource';

type TestNodeOptions = {
	declId?: string;
	nodeType?: string;
	children?: NodeId[];
	tags?: string[];
	fileValue?: string;
};

const testNode = (nodeId: NodeId, options: TestNodeOptions = {}): UiNodeDto =>
	({
		node_id: nodeId,
		uuid: `node-${nodeId}`,
		decl_id: options.declId ?? '',
		node_type: options.nodeType ?? 'test_node',
		children: options.children ?? [],
		meta: { tags: options.tags ?? [] },
		data:
			options.fileValue === undefined
				? { kind: 'node', node_type: options.nodeType ?? 'test_node' }
				: {
						kind: 'parameter',
						param: { value: { kind: 'file', value: options.fileValue } }
					}
	}) as unknown as UiNodeDto;

const nodeMap = (...nodes: UiNodeDto[]): ReadonlyMap<NodeId, UiNodeDto> =>
	new Map(nodes.map((node) => [node.node_id, node]));

const sourceGraph = (rootId: NodeId, ...nodes: UiNodeDto[]): FormulaSourceGraph => {
	const parentById = new Map<NodeId, NodeId>();
	for (const parent of nodes) {
		for (const child of parent.children) parentById.set(child, parent.node_id);
	}
	return { rootId, nodesById: nodeMap(...nodes), parentById };
};

describe('formula source classification', () => {
	it('gives built-in metadata precedence and preserves its source presentation', () => {
		const formula = testNode(1, {
			tags: [
				`${FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX}Action`,
				FORMULA_EXTERNAL_FILE_TAG,
				FORMULA_EXTERNAL_READ_ONLY_TAG
			]
		});

		expect(formulaIsBuiltIn(formula)).toBe(true);
		expect(formulaIsExternalFile(formula)).toBe(true);
		expect(formulaIsReadOnly(formula)).toBe(true);
		expect(formulaSourceKind(formula, sourceGraph(1, formula))).toBe('builtin');
		expect(formulaSourceDisplay('builtin')).toMatchObject({
			badgeLabel: 'Built-in',
			title: 'Built-in formula'
		});
	});

	it('derives the shared folder from Preferences and compares Windows paths case-insensitively', () => {
		const root = testNode(0, { children: [1, 4] });
		const preferences = testNode(1, {
			declId: PREFERENCES_DECL_ID,
			children: [2]
		});
		const saveAndLoad = testNode(2, {
			declId: PREFERENCES_SAVE_AND_LOAD_DECL_ID,
			children: [3]
		});
		const dataFolder = testNode(3, {
			declId: PREFERENCES_DATA_FOLDER_DECL_ID,
			fileValue: 'C:\\Chataigne\\Data\\'
		});
		const formula = testNode(4, {
			children: [5],
			tags: [FORMULA_EXTERNAL_FILE_TAG]
		});
		const externalFile = testNode(5, {
			declId: FORMULA_EXTERNAL_FILE_DECL_ID,
			fileValue: 'c:\\chataigne\\data\\formulas\\Mix.json'
		});
		const graph = sourceGraph(0, root, preferences, saveAndLoad, dataFolder, formula, externalFile);

		expect(sharedFormulaDir(graph, formula)).toBe('C:\\Chataigne\\Data\\formulas');
		expect(formulaExternalFilePath(formula, graph.nodesById)).toBe(
			'c:\\chataigne\\data\\formulas\\Mix.json'
		);
		expect(formulaSourceKind(formula, graph)).toBe('shared');
	});

	it('uses the formula-library folder when Preferences does not expose a data folder', () => {
		const library = testNode(1, {
			nodeType: FORMULA_LIBRARY_NODE_TYPE,
			children: [2, 3]
		});
		const sharedDir = testNode(2, {
			declId: FORMULA_LIBRARY_SHARED_DIR_DECL_ID,
			fileValue: '/srv/chataigne/formulas'
		});
		const formula = testNode(3, {
			children: [4],
			tags: [FORMULA_EXTERNAL_FILE_TAG]
		});
		const externalFile = testNode(4, {
			declId: FORMULA_EXTERNAL_FILE_DECL_ID,
			fileValue: '/srv/chataigne/formulas/Envelope.json'
		});
		const graph = sourceGraph(1, library, sharedDir, formula, externalFile);

		expect(sharedFormulaDir(graph, formula)).toBe('/srv/chataigne/formulas');
		expect(formulaSourceKind(formula, graph)).toBe('shared');
	});

	it('classifies external files outside the shared folder as project formulas', () => {
		const library = testNode(1, {
			nodeType: FORMULA_LIBRARY_NODE_TYPE,
			children: [2, 3]
		});
		const sharedDir = testNode(2, {
			declId: FORMULA_LIBRARY_SHARED_DIR_DECL_ID,
			fileValue: '/srv/chataigne/formulas'
		});
		const formula = testNode(3, {
			children: [4],
			tags: [FORMULA_EXTERNAL_FILE_TAG]
		});
		const externalFile = testNode(4, {
			declId: FORMULA_EXTERNAL_FILE_DECL_ID,
			fileValue: '/project/formulas/Local.json'
		});
		const graph = sourceGraph(1, library, sharedDir, formula, externalFile);

		expect(formulaSourceKind(formula, graph)).toBe('project');
	});

	it('treats a plain formula as project-owned and does not invent an external path', () => {
		const formula = testNode(1);
		const graph = sourceGraph(1, formula);

		expect(formulaExternalFilePath(formula, graph.nodesById)).toBeNull();
		expect(formulaSourceKind(formula, graph)).toBe('project');
	});

	it('resolves the shared folder without enumerating the graph node map', () => {
		const root = testNode(0, { children: [1] });
		const preferences = testNode(1, { declId: PREFERENCES_DECL_ID, children: [2] });
		const saveAndLoad = testNode(2, {
			declId: PREFERENCES_SAVE_AND_LOAD_DECL_ID,
			children: [3]
		});
		const dataFolder = testNode(3, {
			declId: PREFERENCES_DATA_FOLDER_DECL_ID,
			fileValue: '/srv/chataigne'
		});
		const graph = sourceGraph(0, root, preferences, saveAndLoad, dataFolder);
		Object.defineProperty(graph.nodesById, 'values', {
			value: () => {
				throw new Error('Formula source lookup must not scan the complete graph');
			}
		});

		expect(sharedFormulaDir(graph, null)).toBe('/srv/chataigne/formulas');
	});
});
