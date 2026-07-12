import { registerParameterContextPreviewResolver, type UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import type { ProcessorLaneInspectionDto, StateMachineProtocolBundle } from '../generated';
import { STATE_MACHINE_RUNTIME_PREVIEW_TOPIC } from './formulaOutputPreviewStore.svelte';
import {
	contextKeyId,
	formulaPreviewSessionStore,
	processorPreviewLaneOptions
} from './formulaPreviewSessionStore.svelte';

interface SelectedProcessorLaneInspection {
	inspection: ProcessorLaneInspectionDto;
	laneLabel: string;
}

const runtimePreviewBundle = (): StateMachineProtocolBundle | null => {
	const session = appState.session;
	if (!session) return null;
	session.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_TOPIC);
	return session.getCustomEventPayload<StateMachineProtocolBundle>(
		STATE_MACHINE_RUNTIME_PREVIEW_TOPIC
	);
};

const processorAncestor = (node: UiNodeDto): UiNodeDto | null => {
	const graph = appState.session?.graph.state;
	if (!graph) return null;
	let current: UiNodeDto | undefined = graph.nodesById.get(node.node_id) ?? node;
	while (current) {
		if (current.node_type === 'state_processor' || current.user_item_kind === 'state_processor') {
			return current;
		}
		const parentId = graph.parentById.get(current.node_id);
		current = parentId === undefined ? undefined : graph.nodesById.get(parentId);
	}
	return null;
};

export const selectedProcessorLaneInspection = (
	node: UiNodeDto
): SelectedProcessorLaneInspection | null => {
	const processor = processorAncestor(node);
	const bundle = runtimePreviewBundle();
	if (!processor || !bundle) return null;
	const lanes = processorPreviewLaneOptions(
		bundle.processor_lanes.filter((lane) => lane.processor_id === processor.uuid)
	);
	const selectedLane = formulaPreviewSessionStore.processorLane(processor.node_id, lanes);
	if (!selectedLane) return null;
	const selectedContextId = contextKeyId(selectedLane.contextKey);
	const inspection = bundle.processor_lane_inspections.find(
		(candidate) =>
			candidate.processor_id === processor.uuid &&
			contextKeyId(candidate.context_key) === selectedContextId
	);
	return inspection ? { inspection, laneLabel: selectedLane.label } : null;
};

export const selectedLaneConditionValid = (node: UiNodeDto): boolean | null => {
	const selected = selectedProcessorLaneInspection(node);
	return (
		selected?.inspection.condition_states.find((state) => state.node_id === node.uuid)?.valid ??
		null
	);
};

let registered = false;

export const registerProcessorLaneParameterPreviews = (): void => {
	if (registered) return;
	registered = true;
	registerParameterContextPreviewResolver((node) => {
		if (node.data.kind !== 'parameter') return null;
		const mode = node.data.param.control.mode;
		if (mode !== 'contextLink' && mode !== 'templateText') return null;
		const selected = selectedProcessorLaneInspection(node);
		const preview = selected?.inspection.parameter_values.find(
			(candidate) => candidate.node_id === node.uuid
		);
		if (!selected || !preview) return null;
		return {
			text: preview.value,
			label: selected.laneLabel,
			placement: mode === 'templateText' ? 'below' : 'value'
		};
	});
};
