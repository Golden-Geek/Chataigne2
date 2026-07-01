<script lang="ts">
	import {
		showPanel,
		ReferenceEditor,
		type NodeInspectorComponentProps,
		type UiNodeDto
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import formulaIconUrl from '../../golden_alchemist_ui/icons/formula.svg';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);

	let formulaRefNode = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		for (const childId of node.children) {
			const child = graphState.nodesById.get(childId);
			if (
				child?.decl_id === 'formula' &&
				child.data.kind === 'parameter' &&
				child.data.param.value.kind === 'reference'
			) {
				return child;
			}
		}
		return null;
	});

	let formulaNode = $derived.by((): UiNodeDto | null => {
		if (!graphState || formulaRefNode?.data.kind !== 'parameter') return null;
		const value = formulaRefNode.data.param.value;
		if (value.kind !== 'reference') return null;
		if (value.cached_id !== undefined) {
			const cached = graphState.nodesById.get(value.cached_id);
			if (cached?.uuid === value.uuid) return cached;
		}
		for (const n of graphState.nodesById.values()) {
			if (n.uuid === value.uuid) return n;
		}
		return null;
	});

	let formulaKind = $derived(formulaNode?.meta.label ?? 'No Formula');

	const openAlchemistEditor = (): void => {
		showPanel({
			panelId: 'alchemist-editor',
			panelType: 'alchemistEditor',
			title: `Alchemist: ${node.meta.label}`,
			params: formulaNode
				? { formulaNodeId: formulaNode.node_id, processorNodeId: node.node_id }
				: { processorNodeId: node.node_id },
			position: {
				referencePanelId: 'state-machine',
				direction: 'within'
			}
		});
	};
</script>

{#snippet formulaHeaderExtra()}
	<span class="processor-formula-header-extra">
		{#if formulaRefNode}
			<span
				class="formula-reference-control"
				role="presentation"
				onclick={(e) => e.stopPropagation()}
				onkeydown={(e) => e.stopPropagation()}>
				<ReferenceEditor node={formulaRefNode} />
			</span>
		{/if}
		<button
			type="button"
			class="formula-open-btn"
			onclick={(e) => {
				e.stopPropagation();
				openAlchemistEditor();
			}}
			title="Edit Formula">
			<img src={formulaIconUrl} alt="" class="formula-icon" />
			<span class="formula-open-label">Edit Formula</span>
		</button>
	</span>
{/snippet}

{@render defaultHeader?.(formulaHeaderExtra)}

{#snippet formulaContent()}
	{@render defaultChildren?.()}
{/snippet}

{@render defaultContent?.(formulaContent, 'processor-formula-content')}

<style>
	.processor-formula-header-extra {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		flex: 0 0 auto;
		margin-inline-start: 0.2rem;
	}

	.formula-reference-control {
		display: inline-flex;
		align-items: center;
		inline-size: min(14rem, 40vw);
		min-inline-size: 6rem;
		block-size: 1.4rem;
		overflow: hidden;
	}

	.formula-reference-control :global(.reference-editor) {
		inline-size: 100%;
		min-inline-size: 0;
	}

	/* .formula-kind {
		padding: 0.12rem 0.34rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-accent) 45%, transparent);
		border-radius: 999rem;
		color: var(--gc-color-accent);
		font-size: 0.62rem;
		font-weight: 650;
		letter-spacing: 0.04em;
		text-transform: uppercase;
	} */

	.formula-open-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.28rem;
		padding: 0.12rem 0.28rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.65rem;
		cursor: pointer;
		overflow: hidden;
		transition: background 0.12s ease;
	}

	.formula-open-btn:hover {
		background: color-mix(in srgb, var(--gc-color-accent) 18%, transparent);
	}

	.formula-icon {
		display: block;
		width: 0.85rem;
		height: 0.85rem;
		flex: 0 0 auto;
	}

	.formula-open-label {
		max-width: 0;
		overflow: hidden;
		white-space: nowrap;
		opacity: 0;
		transition:
			max-width 0.18s ease-out,
			opacity 0.18s ease-out;
	}

	.formula-open-btn:hover .formula-open-label {
		max-width: 6rem;
		opacity: 1;
	}

	:global(.processor-formula-content) {
		min-inline-size: 0;
	}
</style>
