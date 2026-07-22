<script lang="ts">
	import type { NodeInspectorComponentProps, UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import { selectedLaneConditionValid } from '../preview/processorLaneInspection.svelte';
	import ValidationChip from './ValidationChip.svelte';

	let { node, defaultHeader, defaultContent, defaultChildren }: NodeInspectorComponentProps =
		$props();

	let graphState = $derived(appState.session?.graph.state ?? null);
	let liveNode: UiNodeDto = $derived(graphState?.nodesById.get(node.node_id) ?? node);

	const childByDeclId = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graphState) return null;
		for (const childId of parent.children) {
			const child = graphState.nodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	let validNode = $derived(childByDeclId(liveNode, 'valid'));
	let laneValid = $derived(selectedLaneConditionValid(liveNode));
	let conditionManagerValid = $derived(
		laneValid ??
			(validNode?.data.kind === 'parameter' && validNode.data.param.value.kind === 'bool'
				? validNode.data.param.value.value
				: false)
	);
</script>

{#snippet conditionManagerHeaderExtra()}
	<span
		class="condition-manager-header-extra"
		role="presentation"
		onclick={(event) => event.stopPropagation()}
		onkeydown={(event) => event.stopPropagation()}>
		<ValidationChip
			valid={conditionManagerValid}
			title={conditionManagerValid ? 'Condition manager valid' : 'Condition manager invalid'} />
	</span>
{/snippet}

{@render defaultHeader?.(conditionManagerHeaderExtra)}

{#snippet conditionManagerContent()}
	{@render defaultChildren?.()}
{/snippet}

{@render defaultContent?.(conditionManagerContent, 'condition-manager-inspector')}

<style>
	.condition-manager-header-extra {
		display: inline-flex;
		align-items: center;
		min-inline-size: 0;
		margin-left: 0.2rem;
	}

	:global(.condition-manager-inspector) {
		min-inline-size: 0;
	}
</style>
