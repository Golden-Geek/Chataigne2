<script lang="ts">
	import { ManagerListPanel, type PanelProps, type PanelState, type UiNodeDto } from 'golden_ui';
	import FormulaLibraryRowSupplement from './FormulaLibraryRowSupplement.svelte';

	let _props: PanelProps = $props();

	const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';
	const FORMULA_NODE_TYPE = 'alchemist_formula';
	const FORMULA_FOLDER_NODE_TYPE = 'alchemist_formula_folder';
	const FORMULA_ITEM_KIND = 'alchemist_formula';
	const FORMULA_FOLDER_ITEM_KIND = 'alchemist_formula_folder';

	const isFormulaTreeNode = (candidate: UiNodeDto | null): candidate is UiNodeDto =>
		Boolean(
			candidate &&
			((candidate.node_type === FORMULA_NODE_TYPE &&
				candidate.user_item_kind === FORMULA_ITEM_KIND) ||
				(candidate.node_type === FORMULA_FOLDER_NODE_TYPE &&
					candidate.user_item_kind === FORMULA_FOLDER_ITEM_KIND))
		);
	const canRenderFormulaChildren = (candidate: UiNodeDto): boolean =>
		candidate.node_type === FORMULA_FOLDER_NODE_TYPE;

	export const setPanelState = (_next: PanelState): void => {};
</script>

<ManagerListPanel
	managerNodeType={FORMULA_LIBRARY_NODE_TYPE}
	searchPlaceholder="Search formulas..."
	missingMessage="Formula Library not available."
	emptyMessage="No formulas defined."
	rootDropMessage="Drop here to move into Formulas."
	addButtonTitle="Add formula"
	isTreeNode={isFormulaTreeNode}
	canRenderNodeChildren={canRenderFormulaChildren}
	rowSupplementComponent={FormulaLibraryRowSupplement} />
