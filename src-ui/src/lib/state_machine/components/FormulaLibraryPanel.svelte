<script lang="ts">
	import { ManagerListPanel, type PanelProps, type PanelState, type UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		formulaSourceDisplay,
		formulaSourceKind,
		type FormulaSourceKind,
		type FormulaSourceDisplay
	} from '../formulaSource';
	import FormulaLibraryRowSupplement from './FormulaLibraryRowSupplement.svelte';

	let _props: PanelProps = $props();

	const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';
	const FORMULA_NODE_TYPE = 'alchemist_formula';
	const FORMULA_FOLDER_NODE_TYPE = 'alchemist_formula_folder';
	const FORMULA_ITEM_KIND = 'alchemist_formula';
	const FORMULA_FOLDER_ITEM_KIND = 'alchemist_formula_folder';

	type SourceFilter = { id: FormulaSourceKind } & FormulaSourceDisplay;

	const SOURCE_FILTERS: SourceFilter[] = (['builtin', 'shared', 'project'] as const).map((id) => ({
		id,
		...formulaSourceDisplay(id)
	}));

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

	let activeSources = $state<Set<FormulaSourceKind>>(
		new Set(SOURCE_FILTERS.map((filter) => filter.id))
	);

	const toggleSource = (id: FormulaSourceKind): void => {
		const next = new Set(activeSources);
		if (next.has(id)) {
			next.delete(id);
		} else {
			next.add(id);
		}
		activeSources = next;
	};

	let nodesById = $derived(appState.session?.graph.state.nodesById ?? new Map());

	// Folders always pass through; only formula leaves are filtered by source,
	// so a folder containing a mix of sources stays browsable.
	const matchesActiveSource = (candidate: UiNodeDto): boolean =>
		candidate.node_type !== FORMULA_NODE_TYPE ||
		activeSources.has(formulaSourceKind(candidate, nodesById));

	export const setPanelState = (_next: PanelState): void => {};
</script>

<div class="formula-library-panel">
	<div class="formula-library-list">
		<ManagerListPanel
			managerNodeType={FORMULA_LIBRARY_NODE_TYPE}
			searchPlaceholder="Search formulas..."
			missingMessage="Formula Library not available."
			emptyMessage="No formulas defined."
			rootDropMessage="Drop here to move into Formulas."
			addButtonTitle="Add formula"
			isTreeNode={isFormulaTreeNode}
			canRenderNodeChildren={canRenderFormulaChildren}
			extraNodeFilter={matchesActiveSource}
			rowSupplementComponent={FormulaLibraryRowSupplement} />
	</div>
	<div class="formula-source-filter" role="group" aria-label="Filter by formula source">
		{#each SOURCE_FILTERS as filter (filter.id)}
			<button
				type="button"
				class="formula-source-chip"
				class:active={activeSources.has(filter.id)}
				style:--formula-source-color={filter.accent}
				title={filter.title}
				aria-pressed={activeSources.has(filter.id)}
				onclick={() => toggleSource(filter.id)}>
				{filter.filterLabel}
			</button>
		{/each}
	</div>
</div>

<style>
	.formula-library-panel {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-block-size: 0;
	}

	.formula-source-filter {
		display: flex;
		gap: 0.3rem;
		padding: 0.35rem 0.35rem 0;
	}

	.formula-source-chip {
		padding: 0.18rem 0.5rem;
		flex: 1;
		border: 0.06rem solid
			color-mix(in srgb, var(--formula-source-color) 36%, var(--gc-color-border));
		border-radius: 999rem;
		background: color-mix(in srgb, var(--formula-source-color) 10%, transparent);
		color: color-mix(in srgb, var(--formula-source-color) 62%, var(--gc-color-text));
		font-size: 0.68rem;
		font-weight: 650;
		line-height: 1.3;
		white-space: nowrap;
		cursor: pointer;
		transition:
			background 0.1s ease-in-out,
			border-color 0.1s ease-in-out,
			color 0.1s ease-in-out;
	}

	.formula-source-chip.active {
		color: var(--gc-color-text);
		background: color-mix(in srgb, var(--formula-source-color) 38%, transparent);
		border-color: color-mix(in srgb, var(--formula-source-color) 72%, transparent);
	}

	.formula-library-list {
		flex: 1;
		min-block-size: 0;
	}
</style>
