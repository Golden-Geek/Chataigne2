<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import type { StateMachinePreviewCatalogDto } from '../../state_machine/generated';
	import { STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC } from '../preview/formulaOutputPreviewStore.svelte';
	import {
		formulaPreviewSessionStore,
		processorPreviewLaneOptions
	} from '../preview/formulaPreviewSessionStore.svelte';
	import { selectedLaneConditionValid } from '../preview/processorLaneInspection.svelte';
	import ValidationChip from './ValidationChip.svelte';

	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PROCESSOR_MANAGED_REGIONS_DECL_ID = 'managed_regions';
	const CONDITION_MANAGER_NODE_TYPE = 'sm_condition_manager';
	const CONDITION_MANAGER_VALID_DECL_ID = 'valid';

	let { node } = $props<{
		node: UiNodeDto;
	}>();

	let session = $derived(appState.session);
	let runtimePreviewCatalogSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC) ?? 0
	);
	let runtimePreviewCatalog = $derived.by((): StateMachinePreviewCatalogDto | null => {
		runtimePreviewCatalogSequence;
		if (!session) return null;
		return (
			session.getCustomEventPayload<StateMachinePreviewCatalogDto>(
				STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC
			) ?? null
		);
	});
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
		const laneValid = selectedLaneConditionValid(manager);
		if (laneValid !== null) return laneValid;
		const valid = directChild(manager, CONDITION_MANAGER_VALID_DECL_ID);
		return valid?.data.kind === 'parameter' && valid.data.param.value.kind === 'bool'
			? valid.data.param.value.value
			: false;
	};

	let conditionManagers = $derived(isProcessorNode ? processorConditionManagers(liveNode) : []);
	let multiplexLaneCount = $derived.by((): number => {
		if (!isProcessorNode || !runtimePreviewCatalog) return 0;
		return (
			runtimePreviewCatalog.processors.find((processor) => processor.id === liveNode.uuid)
				?.multiplex_lane_count ?? 0
		);
	});
	let previewLanes = $derived(
		processorPreviewLaneOptions(
			runtimePreviewCatalog?.processor_lanes.filter((lane) => lane.processor_id === liveNode.uuid) ??
				[]
		)
	);
	let previewLane = $derived(
		isProcessorNode
			? formulaPreviewSessionStore.processorLane(liveNode.node_id, previewLanes)
			: null
	);
</script>

{#if conditionManagers.length > 0 || multiplexLaneCount > 0 || previewLane}
	<span class="processor-row-supplements">
		{#if multiplexLaneCount > 0}
			<span class="processor-multiplex-badge" title={`${multiplexLaneCount} multiplex lanes`}>
				M : {multiplexLaneCount}
			</span>
		{/if}

		{#if multiplexLaneCount > 1 && previewLane}
			<span class="processor-preview-lane" title={`Preview lane: ${previewLane.label}`}>
				{previewLane.label}
			</span>
		{/if}
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
	.processor-row-supplements {
		display: inline-flex;
		justify-content: end;
		align-items: center;
		gap: 0.25rem;
		flex: 1 0 auto;
	}

	.processor-multiplex-badge {
		display: inline-flex;
		align-items: center;
		min-height: 1.25em;
		padding: 0 0.35em;
		border: 0.0625rem solid color-mix(in srgb, currentColor 28%, transparent);
		border-radius: 0.25rem;
		font-size: 0.72em;
		font-weight: 650;
		line-height: 1.2;
		white-space: nowrap;
	}

	.processor-preview-lane {
		display: inline-block;
		max-inline-size: 12rem;
		overflow: hidden;
		color: color-mix(in srgb, var(--gc-color-text) 76%, transparent);
		font-size: 0.72em;
		font-weight: 600;
		line-height: 1.2;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
</style>
