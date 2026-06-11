<script lang="ts">
	import { NodeAddButton, OutlinerItem, type UiNodeDto } from 'golden_ui';
	import type { PanelProps, PanelState } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';

	const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';

	let props: PanelProps = $props();
	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);

	export const setPanelState = (_next: PanelState): void => {};

	let formulaLibrary = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		for (const node of graphState.nodesById.values()) {
			if (node.node_type === FORMULA_LIBRARY_NODE_TYPE) return node;
		}
		return null;
	});

	let liveLibrary = $derived(
		formulaLibrary ? (graphState?.nodesById.get(formulaLibrary.node_id) ?? formulaLibrary) : null
	);

	let formulas = $derived.by((): UiNodeDto[] => {
		if (!liveLibrary || !graphState) return [];
		return liveLibrary.children
			.map((id) => graphState.nodesById.get(id))
			.filter((n): n is UiNodeDto => n != null);
	});
</script>

<div class="formula-library-panel">
	<div class="panel-header">
		<span class="panel-title">Formulas</span>
		{#if liveLibrary}
			<NodeAddButton node={liveLibrary} />
		{/if}
	</div>

	{#if !liveLibrary}
		<p class="empty">Formula Library not available.</p>
	{:else if formulas.length === 0}
		<p class="empty">No formulas defined.</p>
	{:else}
		<div class="formula-list">
			{#each formulas as formula (formula.node_id)}
				<div class="formula-row">
					<div class="formula-item">
						<OutlinerItem node={formula} initiallyExpandedDepth={0} />
					</div>
					<span class="kind-badge">Formula</span>
				</div>
			{/each}
		</div>
	{/if}
</div>

<style>
	.formula-library-panel {
		display: flex;
		flex-direction: column;
		block-size: 100%;
		min-block-size: 0;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
		overflow: hidden;
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		min-block-size: 2rem;
		padding-inline: 0.55rem 0.25rem;
		border-block-end: 0.06rem solid var(--gc-color-border);
		background: var(--gc-color-background-soft);
	}

	.panel-title {
		font-size: 0.75rem;
		font-weight: 600;
	}

	.formula-list {
		flex: 1 1 auto;
		overflow-y: auto;
		padding: 0.25rem;
	}

	.formula-row {
		display: flex;
		align-items: center;
		gap: 0.4rem;
	}

	.formula-item {
		flex: 1 1 auto;
		min-inline-size: 0;
	}

	.kind-badge {
		flex: 0 0 auto;
		padding: 0.1rem 0.35rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-text) 25%, transparent);
		border-radius: 999rem;
		color: color-mix(in srgb, var(--gc-color-text) 55%, transparent);
		font-size: 0.58rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.04em;
	}

	.empty {
		margin: 0.75rem;
		color: color-mix(in srgb, var(--gc-color-text) 55%, transparent);
		font-size: 0.72rem;
		text-align: center;
	}
</style>
