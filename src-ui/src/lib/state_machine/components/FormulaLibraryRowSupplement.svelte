<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import {
		FORMULA_EXTERNAL_FILE_DECL_ID,
		formulaIsExternalFile,
		formulaSourceDisplay,
		formulaSourceKind,
		sharedFormulaDir
	} from '../formulaSource';

	// Must match FORMULA_EXTERNAL_FILE_CREATE_TYPE in
	// src/state_machine_nodes/formula.rs — this is the same create-type the
	// "External Formula" Add-menu item uses, just with the path pre-filled.
	const FORMULA_EXTERNAL_FILE_CREATE_TYPE = 'alchemist_formula:external_file';
	const FORMULA_NODE_TYPE = 'alchemist_formula';
	const FORMULA_EXTERNAL_SOURCE_DECL_ID = 'external_formula_source';
	const FORMULA_COPY_SOURCE_DECL_ID = 'formula_copy_source';

	let { node }: { node: UiNodeDto } = $props();

	let nodesById = $derived(appState.session?.graph.state.nodesById ?? new Map());
	let sourceKind = $derived(formulaSourceKind(node, nodesById));
	let sourceDisplay = $derived(formulaSourceDisplay(sourceKind));
	let external = $derived(formulaIsExternalFile(node));
	let sourceTitle = $derived(
		external && sourceKind === 'project'
			? 'Project formula linked to an external file'
			: sourceDisplay.title
	);
	let sharedDir = $derived(sharedFormulaDir(nodesById));
	// Only a plain, in-project-authored formula makes sense to publish to the
	// cross-project Shared folder; built-ins/shared/external-file-linked ones
	// already have a canonical source elsewhere.
	let canSaveToShared = $derived(
		sourceKind === 'project' && !external && sharedDir !== null
	);
	let canCopyToProject = $derived(sourceKind === 'shared' || sourceKind === 'builtin');

	let saving = $state(false);
	let savedFeedback = $state(false);
	let saveError = $state<string | null>(null);

	const sharedFormulaStem = (label: string): string => {
		const trimmed = label.trim();
		const stem = (trimmed.length > 0 ? trimmed : 'formula')
			.replace(/[<>:"/\\|?* -]+/g, '-')
			.replace(/\s+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 80);
		return (stem || 'formula').toLowerCase();
	};

	const joinPath = (directory: string, fileName: string): string => {
		const separator = directory.includes('\\') ? '\\' : '/';
		return `${directory.replace(/[\\/]+$/g, '')}${separator}${fileName}`;
	};

	const referenceValue = (target: UiNodeDto) => ({
		kind: 'reference' as const,
		uuid: target.uuid,
		cached_id: target.node_id,
		cached_name: target.meta.label,
		relative_path_from_root: [] as string[]
	});

	const saveToShared = async (event: MouseEvent): Promise<void> => {
		event.stopPropagation();
		const graphState = appState.session?.graph.state;
		const parentId = graphState?.parentById.get(node.node_id);
		if (!graphState || parentId === undefined || saving) {
			return;
		}
		saving = true;
		saveError = null;
		try {
			if (!sharedDir) {
				saveError = 'Shared formulas folder is not configured.';
				return;
			}
			const stem = sharedFormulaStem(node.meta.label);
			const targetPath = joinPath(sharedDir, `${stem}.json`);

			// Adds a new Shared sibling next to this formula (via the same
			// creation path as the "External Formula" Add-menu item, pointed
			// at the file the backend will write) rather than converting this
			// node in place, so any processor already using this formula keeps
			// working.
			const result = await sendCreateUserItemByTypeIntent(
				parentId,
				FORMULA_EXTERNAL_FILE_CREATE_TYPE,
				node.meta.label,
				{
					created_node_type: FORMULA_NODE_TYPE,
					initial_params: [
						{
							decl_id: FORMULA_EXTERNAL_FILE_DECL_ID,
							value: { kind: 'file', value: targetPath }
						},
						{
							decl_id: FORMULA_EXTERNAL_SOURCE_DECL_ID,
							value: referenceValue(node)
						}
					]
				}
			);
			if (!result.success) {
				saveError = 'Save failed - see console for details.';
				return;
			}

			savedFeedback = true;
			setTimeout(() => {
				savedFeedback = false;
			}, 1500);
		} finally {
			saving = false;
		}
	};

	const copyToProject = async (event: MouseEvent): Promise<void> => {
		event.stopPropagation();
		const graphState = appState.session?.graph.state;
		const parentId = graphState?.parentById.get(node.node_id);
		if (!graphState || parentId === undefined || saving) {
			return;
		}
		saving = true;
		saveError = null;
		try {
			const result = await sendCreateUserItemByTypeIntent(parentId, FORMULA_NODE_TYPE, node.meta.label, {
				created_node_type: FORMULA_NODE_TYPE,
				initial_params: [
					{
						decl_id: FORMULA_COPY_SOURCE_DECL_ID,
						value: referenceValue(node)
					}
				]
			});
			if (!result.success) {
				saveError = 'Copy failed - see console for details.';
				return;
			}
			savedFeedback = true;
			setTimeout(() => {
				savedFeedback = false;
			}, 1500);
		} finally {
			saving = false;
		}
	};

</script>

<span class="formula-row-supplement">
	<span
		class="formula-source-pill"
		title={sourceTitle}
		style:--formula-source-color={sourceDisplay.accent}>
		{sourceDisplay.badgeLabel}
	</span>
	{#if canSaveToShared}
		<button
			type="button"
			class="save-to-shared-button"
			title={saveError ?? 'Publish a copy to your Shared formulas folder, reusable across projects'}
			class:has-error={saveError !== null}
			disabled={saving}
			onclick={saveToShared}>
			{#if savedFeedback}
				Added to Shared
			{:else if saveError}
				Save failed
			{:else}
				Save to Shared
			{/if}
		</button>
	{/if}
	{#if canCopyToProject}
		<button
			type="button"
			class="save-to-shared-button"
			title={saveError ?? 'Copy this formula into the current project'}
			class:has-error={saveError !== null}
			disabled={saving}
			onclick={copyToProject}>
			{savedFeedback ? 'Copied' : 'Copy to Project'}
		</button>
	{/if}
</span>

<style>
	.formula-row-supplement {
		display: inline-flex;
		align-items: center;
		justify-content: flex-end;
		gap: 0.25rem;
	}

	.formula-source-pill {
		display: inline-flex;
		align-items: center;
		min-block-size: 1rem;
		padding: 0.08rem 0.28rem;
		border: 0.06rem solid
			color-mix(in srgb, var(--formula-source-color) 48%, var(--gc-color-border));
		border-radius: 999rem;
		background: color-mix(in srgb, var(--formula-source-color) 16%, transparent);
		color: color-mix(in srgb, var(--formula-source-color) 62%, var(--gc-color-text));
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
	}

	.save-to-shared-button {
		display: inline-flex;
		align-items: center;
		min-block-size: 1rem;
		padding: 0.08rem 0.32rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 75%, transparent);
		border-radius: 999rem;
		background: transparent;
		color: color-mix(in srgb, var(--gc-color-text) 66%, transparent);
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
		cursor: pointer;
	}

	.save-to-shared-button:hover:not(:disabled) {
		border-color: var(--gc-color-accent, #5d8cff);
		color: var(--gc-color-text);
	}

	.save-to-shared-button:disabled {
		cursor: default;
		opacity: 0.6;
	}

	.save-to-shared-button.has-error {
		border-color: color-mix(in srgb, #ff5c5c 60%, transparent);
		color: #ff8b8b;
	}
</style>
