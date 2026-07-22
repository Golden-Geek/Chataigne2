import type { UiNodeDto } from 'golden_ui';
import { appState } from 'golden_ui/store/workbench.svelte';
import type {
	ContextKeyDto,
	ProcessorOverviewLaneSelectionDto,
	ProcessorRuntimeOverviewDto,
	StateMachineProcessorOverviewDto
} from './generated';

export const STATE_MACHINE_PROCESSOR_OVERVIEW_TOPIC =
	'chataigne.state_machine.processor_overview';
export const STATE_MACHINE_PROCESSOR_OVERVIEW_DEMAND_TOPIC =
	'chataigne.state_machine.processor_overview_demand';
export const STATE_MACHINE_PROCESSOR_OVERVIEW_LANE_TOPIC =
	'chataigne.state_machine.processor_overview_lane';

let cachedSession: unknown = null;
let cachedSequences = new Map<string, number>();
let cachedProcessors = new Map<string, Map<string, ProcessorRuntimeOverviewDto>>();

export const processorOverviewTopic = (processorId: string): string => {
	const shard = /^[0-9a-fA-F]{2}/.exec(processorId)?.[0]?.toLowerCase() ?? '00';
	return `${STATE_MACHINE_PROCESSOR_OVERVIEW_TOPIC}.${shard}`;
};

export const processorRuntimeOverview = (
	processorId: string
): ProcessorRuntimeOverviewDto | null => {
	const session = appState.session;
	if (!session) return null;
	if (cachedSession !== session) {
		cachedSession = session;
		cachedSequences = new Map();
		cachedProcessors = new Map();
	}
	const topic = processorOverviewTopic(processorId);
	const sequence = session.getCustomEventSequence(topic);
	if (cachedSequences.get(topic) !== sequence) {
		const overview = session.getCustomEventPayload<StateMachineProcessorOverviewDto>(
			topic
		);
		cachedSequences.set(topic, sequence);
		cachedProcessors.set(
			topic,
			new Map(overview?.processors.map((processor) => [processor.processor_id, processor]) ?? [])
		);
	}
	return cachedProcessors.get(topic)?.get(processorId) ?? null;
};

export const publishProcessorOverviewLaneSelection = (
	processor: UiNodeDto,
	contextKey: ContextKeyDto | null
): void => {
	const session = appState.session;
	const graph = session?.graph.state;
	if (!session || session.status !== 'connected' || !graph) return;
	const manager = [...graph.nodesById.values()].find(
		(node) => node.node_type === 'state_machine_manager'
	);
	if (!manager) return;
	const payload: ProcessorOverviewLaneSelectionDto = {
		processor_id: processor.uuid,
		context_key: contextKey
	};
	void session
		.sendIntent({
			kind: 'sendNodeEvent',
			node: manager.node_id,
			topic: STATE_MACHINE_PROCESSOR_OVERVIEW_LANE_TOPIC,
			payload
		})
		.catch(() => undefined);
};
