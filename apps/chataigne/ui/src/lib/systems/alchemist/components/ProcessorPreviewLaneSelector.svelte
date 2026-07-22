<script lang="ts">
	import type { UiNodeDto } from 'golden_ui';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import type { StateMachinePreviewCatalogDto } from '../../state_machine/generated';
	import { STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC } from '../preview/formulaOutputPreviewStore.svelte';
	import {
		formulaPreviewSessionStore,
		processorPreviewLaneOptions
	} from '../preview/formulaPreviewSessionStore.svelte';
	import ProcessorLaneSelector from './ProcessorLaneSelector.svelte';

	let { node }: { node: UiNodeDto } = $props();

	let session = $derived(appState.session);
	let runtimePreviewCatalogSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC) ?? 0
	);
	let runtimePreviewCatalog = $derived.by((): StateMachinePreviewCatalogDto | null => {
		runtimePreviewCatalogSequence;
		return (
			session?.getCustomEventPayload<StateMachinePreviewCatalogDto>(
				STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC
			) ?? null
		);
	});
	let lanes = $derived(
		processorPreviewLaneOptions(
			runtimePreviewCatalog?.processor_lanes.filter((lane) => lane.processor_id === node.uuid) ?? []
		)
	);
	let selectedLane = $derived(formulaPreviewSessionStore.processorLane(node.node_id, lanes));
</script>

<span
	class="processor-preview-lane-selector"
	role="presentation"
	onclick={(event) => event.stopPropagation()}
	onkeydown={(event) => event.stopPropagation()}>
	<ProcessorLaneSelector
		{lanes}
		selectedLaneId={selectedLane?.id ?? null}
		onSelect={(laneId) => formulaPreviewSessionStore.selectProcessorLane(node.node_id, laneId)} />
</span>

<style>
	.processor-preview-lane-selector {
		display: inline-flex;
		align-items: center;
		min-inline-size: 0;
	}

	.processor-preview-lane-selector :global(.lane-selector select) {
		max-inline-size: 12rem;
	}
</style>
