<script lang="ts">
	import type { NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import ReferencedParameterHeaderControl from './ReferencedParameterHeaderControl.svelte';

	let { node, defaultHeader, defaultChildren, collapsed }: NodeInspectorComponentProps = $props();

	let session = $derived(appState.session);
	let graphNodesById = $derived(session?.graph.state.nodesById ?? null);
	let liveNode: UiNodeDto = $derived(graphNodesById?.get(node.node_id) ?? node);

	const childByDeclId = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphNodesById) return null;
		for (const childId of parent.children) {
			const child = graphNodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	let validNode = $derived(childByDeclId(liveNode, 'valid'));
	let conditionValid = $derived(
		validNode?.data.kind === 'parameter' && validNode.data.param.value.kind === 'bool'
			? validNode.data.param.value.value
			: false
	);
</script>

{#snippet conditionHeaderExtra()}
	<span
		class="condition-header-extra"
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<span
			class:valid={conditionValid}
			class="condition-validity"
			title={conditionValid ? 'Condition valid' : 'Condition invalid'}>
			{conditionValid ? 'Valid' : 'Invalid'}
		</span>
		<ReferencedParameterHeaderControl owner={liveNode} />
	</span>
{/snippet}

{@render defaultHeader?.(conditionHeaderExtra)}

{#if collapsed !== true}
	<div class="node-inspector-content input-value-condition-inspector">
		{@render defaultChildren?.()}
	</div>
{/if}

<style>
	.condition-header-extra {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		min-inline-size: 0;
		margin-left: 0.2rem;
	}

	.condition-validity {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		block-size: 1.2rem;
		min-inline-size: 3.4rem;
		padding-inline: 0.45rem;
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-danger, #d85252) 18%, transparent);
		color: var(--gc-danger, #d85252);
		font-size: 0.68rem;
		font-weight: 700;
		line-height: 1;
	}

	.condition-validity.valid {
		background: color-mix(in srgb, var(--gc-success, #47a66a) 18%, transparent);
		color: var(--gc-success, #47a66a);
	}

	.input-value-condition-inspector {
		min-inline-size: 0;
	}
</style>
