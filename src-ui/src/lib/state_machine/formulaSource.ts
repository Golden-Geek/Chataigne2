import type { NodeId, UiNodeDto } from 'golden_ui';

export const FORMULA_EXTERNAL_FILE_TAG = 'chataigne.formula.external.file';
export const FORMULA_EXTERNAL_FILE_DECL_ID = 'external_formula_file';
export const FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX = 'chataigne.formula.external.builtin:';
export const FORMULA_EXTERNAL_READ_ONLY_TAG = 'chataigne.formula.external.read_only';
export const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';
export const FORMULA_LIBRARY_SHARED_DIR_DECL_ID = 'shared_formula_dir';

export type FormulaSourceKind = 'builtin' | 'shared' | 'project';

const hasTag = (node: UiNodeDto | null | undefined, tag: string): boolean =>
	Boolean(node?.meta.tags.includes(tag));

const hasTagPrefix = (node: UiNodeDto | null | undefined, prefix: string): boolean =>
	Boolean(node?.meta.tags.some((candidate) => candidate.startsWith(prefix)));

export const formulaIsBuiltIn = (node: UiNodeDto | null | undefined): boolean =>
	hasTagPrefix(node, FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX);

export const formulaIsExternalFile = (node: UiNodeDto | null | undefined): boolean =>
	hasTag(node, FORMULA_EXTERNAL_FILE_TAG);

export const formulaIsReadOnly = (node: UiNodeDto | null | undefined): boolean =>
	hasTag(node, FORMULA_EXTERNAL_READ_ONLY_TAG);

const stringParamValue = (node: UiNodeDto | null | undefined): string | null => {
	if (!node || node.data.kind !== 'parameter') {
		return null;
	}
	const { value } = node.data.param;
	return value.kind === 'file' || value.kind === 'str' ? value.value : null;
};

/** A direct child of `node` with the given decl_id, or null if absent. */
const findChildByDeclId = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	declId: string
): UiNodeDto | null => {
	if (!node) {
		return null;
	}
	for (const childId of node.children) {
		const child = nodesById.get(childId);
		if (child?.decl_id === declId) {
			return child;
		}
	}
	return null;
};

/** This formula's linked file path, or null if it isn't external-file-linked. */
export const formulaExternalFilePath = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): string | null => {
	if (!formulaIsExternalFile(node)) {
		return null;
	}
	const pathParam = findChildByDeclId(node, nodesById, FORMULA_EXTERNAL_FILE_DECL_ID);
	const path = stringParamValue(pathParam);
	return path && path.trim().length > 0 ? path : null;
};

/** The resolved shared-formulas folder, read from the project's Formula Library node. */
export const sharedFormulaDir = (nodesById: ReadonlyMap<NodeId, UiNodeDto>): string | null => {
	for (const node of nodesById.values()) {
		if (node.node_type === FORMULA_LIBRARY_NODE_TYPE) {
			const dirParam = findChildByDeclId(node, nodesById, FORMULA_LIBRARY_SHARED_DIR_DECL_ID);
			const dir = stringParamValue(dirParam);
			return dir && dir.trim().length > 0 ? dir : null;
		}
	}
	return null;
};

const normalizeForPathCompare = (path: string): string => path.replace(/\\/g, '/').toLowerCase();

/**
 * Where a formula's definition comes from, for the Formula Library's
 * Built-ins/Shared/Project filter and for source badges. Shared formulas
 * are external-file-linked formulas (see `formulaIsExternalFile`) whose
 * path happens to be inside the shared formulas folder — there's no
 * separate "shared" mechanism, matching the backend (see
 * `state_machine_nodes::catalog::FormulaCatalog::add_project_formulas`).
 * "Project" covers plain formulas and external-file links pointing
 * anywhere else.
 */
export const formulaSourceKind = (
	node: UiNodeDto | null | undefined,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>
): FormulaSourceKind => {
	if (formulaIsBuiltIn(node)) {
		return 'builtin';
	}
	const filePath = formulaExternalFilePath(node, nodesById);
	if (filePath) {
		const sharedDir = sharedFormulaDir(nodesById);
		if (sharedDir && normalizeForPathCompare(filePath).startsWith(normalizeForPathCompare(sharedDir))) {
			return 'shared';
		}
	}
	return 'project';
};
