<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import {
		buildNodeTreeClipboardPayload,
		nodeTreeClipboardJson
	} from 'golden_ui/store/session/node-tree-clipboard';
	import { hasDesktopHost, writeDesktopAppDataFile } from 'golden_ui/host/desktop';
	import {
		FORMULA_EXTERNAL_FILE_DECL_ID,
		formulaIsExternalFile,
		formulaSourceKind
	} from '../formulaSource';

	const SHARED_FORMULA_APP_DATA_SEGMENTS = ['Chataigne', 'formulas'];
	// Must match FORMULA_EXTERNAL_FILE_CREATE_TYPE in
	// src/state_machine_nodes/formula.rs — this is the same create-type the
	// "External Formula" Add-menu item uses, just with the path pre-filled.
	const FORMULA_EXTERNAL_FILE_CREATE_TYPE = 'alchemist_formula:external_file';

	let { node }: { node: UiNodeDto } = $props();

	let nodesById = $derived(appState.session?.graph.state.nodesById ?? new Map());
	let sourceKind = $derived(formulaSourceKind(node, nodesById));
	let external = $derived(formulaIsExternalFile(node));
	// Only a plain, in-project-authored formula makes sense to publish to the
	// cross-project Shared folder; built-ins/shared/external-file-linked ones
	// already have a canonical source elsewhere.
	let canSaveToShared = $derived(sourceKind === 'project' && !external && hasDesktopHost());

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
			const stem = sharedFormulaStem(node.meta.label);
			const payload = buildNodeTreeClipboardPayload([node], graphState.nodesById);
			const json = nodeTreeClipboardJson(payload);
			const written = await writeDesktopAppDataFile(
				SHARED_FORMULA_APP_DATA_SEGMENTS,
				`${stem}.json`,
				json,
				'save-to-shared'
			);
			if (!written) {
				saveError = 'Save failed - see console for details.';
				return;
			}

			// Adds a new Shared sibling next to this formula (via the same
			// creation path as the "External Formula" Add-menu item, pointed
			// at the file just written) rather than converting this node in
			// place, so any processor already using this formula keeps working.
			const result = await sendCreateUserItemByTypeIntent(
				parentId,
				FORMULA_EXTERNAL_FILE_CREATE_TYPE,
				node.meta.label,
				{
					initial_params: [
						{
							decl_id: FORMULA_EXTERNAL_FILE_DECL_ID,
							value: { kind: 'file', value: written }
						}
					]
				}
			);
			if (!result.success) {
				saveError = `Saved to ${written}, but failed to add it to the library.`;
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

{#if sourceKind === 'builtin'}
	<span class="builtin-pill" title="Built-in formula">Built-in</span>
{:else if sourceKind === 'shared'}
	<span class="builtin-pill" title="Shared formula">Shared</span>
{:else if external}
	<span class="builtin-pill" title="External formula">External</span>
{/if}
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

<style>
	.builtin-pill {
		display: inline-flex;
		align-items: center;
		min-block-size: 1rem;
		padding: 0.08rem 0.28rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 75%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--gc-color-accent, #5d8cff) 12%, transparent);
		color: color-mix(in srgb, var(--gc-color-text) 66%, transparent);
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
