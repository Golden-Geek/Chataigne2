<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import ValidationChip from './ValidationChip.svelte';

	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PROCESSOR_MANAGED_REGIONS_DECL_ID = 'managed_regions';
	const CONDITION_MANAGER_NODE_TYPE = 'sm_condition_manager';
	const CONDITION_MANAGER_VALID_DECL_ID = 'valid';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	let session = $derived(appState.session);
	let graph = $derived(session?.graph.state ?? null);
	let liveNode = $derived(graph?.nodesById.get(node.node_id) ?? node);
	let isProcessorNode = $derived(
		liveNode.user_item_kind === PROCESSOR_ITEM_KIND || liveNode.node_type === PROCESSOR_ITEM_KIND
	);

	const directChild = (parent: UiNodeDto, declId: string): UiNodeDto | null => {
		if (!graph) return null;
		for (const childId of parent.children) {
			const child = graph.nodesById.get(childId);
			if (child?.decl_id === declId) return child;
		}
		return null;
	};

	const processorConditionManagers = (processor: UiNodeDto): UiNodeDto[] => {
		if (!graph) return [];
		const managers: UiNodeDto[] = [];
		const visit = (current: UiNodeDto | null): void => {
			if (!current) return;
			for (const childId of current.children) {
				const child = graph.nodesById.get(childId);
				if (!child) continue;
				// Property surfaces are mirrored flat at the processor's top level;
				// skip the managed-regions subtree so only property condition
				// managers surface a validation chip.
				if (child.decl_id === PROCESSOR_MANAGED_REGIONS_DECL_ID) continue;
				if (child.node_type === CONDITION_MANAGER_NODE_TYPE) {
					managers.push(child);
					continue;
				}
				if (child.children.length > 0) visit(child);
			}
		};
		visit(processor);
		return managers;
	};

	const conditionManagerValid = (manager: UiNodeDto): boolean => {
		const valid = directChild(manager, CONDITION_MANAGER_VALID_DECL_ID);
		return valid?.data.kind === 'parameter' && valid.data.param.value.kind === 'bool'
			? valid.data.param.value.value
			: false;
	};

	let conditionManagers = $derived(isProcessorNode ? processorConditionManagers(liveNode) : []);
</script>

{#if conditionManagers.length > 0}
	<span class="processor-validation-chips" aria-label="Condition manager status">
		{#each conditionManagers as manager (manager.node_id)}
			{@const valid = conditionManagerValid(manager)}
			<ValidationChip
				{valid}
				showLabel={false}
				title={`${manager.meta.label} ${valid ? 'valid' : 'invalid'}`} />
		{/each}
	</span>
{/if}

<style>
	.processor-validation-chips {
		display: inline-flex;
		justify-content: end;
		align-items: center;
		gap: 0.15rem;
		flex: 1 0 auto;
	}
</style>
