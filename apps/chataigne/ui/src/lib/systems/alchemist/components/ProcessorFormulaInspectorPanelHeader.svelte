<script lang="ts">
	import {
		showPanel,
		type NodeInspectorPanelHeaderComponentProps,
		type UiNodeDto
	} from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import formulaIconUrl from '$lib/assets/icons/formula.svg';
	import ProcessorPreviewLaneSelector from './ProcessorPreviewLaneSelector.svelte';

	let { node, defaultHeader }: NodeInspectorPanelHeaderComponentProps = $props();

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);

	let formulaNode = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		for (const childId of node.children) {
			const child = graphState.nodesById.get(childId);
			if (
				child?.decl_id === 'formula' &&
				child.data.kind === 'parameter' &&
				child.data.param.value.kind === 'reference'
			) {
				const reference = child.data.param.value;
				if (reference.cached_id !== undefined) {
					const cached = graphState.nodesById.get(reference.cached_id);
					if (cached?.uuid === reference.uuid) return cached;
				}
				for (const n of graphState.nodesById.values()) {
					if (n.uuid === reference.uuid) return n;
				}
			}
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
		<ProcessorPreviewLaneSelector {node} />
		<!-- <span class="formula-kind">{formulaKind}</span> -->
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

<style>
	.processor-formula-header-extra {
		display: inline-flex;
		align-items: center;
		gap: 0.25rem;
		flex: 0 0 auto;
		margin-inline-start: 0.2rem;
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
</style>
