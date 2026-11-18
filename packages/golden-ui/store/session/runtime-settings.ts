import type { ParamValue, UiNodeDto, UiParameterControlState } from '../../types';
import type { GraphStore } from '../graph.svelte';

const PREFERENCES_DECL_ID = 'preferences';
const INTERFACE_DECL_ID = 'interface';
const SHOW_OSC_ADDRESS_IN_HELP_DECL_ID = 'show_osc_address_in_help';
const ENGINE_DECL_ID = 'engine';
const ENGINE_LOW_FREQUENCY_DECL_ID = 'engine_low_frequency';
const DEFAULT_ENGINE_LOW_FREQUENCY_HZ = 60;

const childByDeclId = (
	graph: GraphStore,
	parent: UiNodeDto | null | undefined,
	declId: string
): UiNodeDto | null => {
	if (!parent) {
		return null;
	}
	for (const childId of parent.children) {
		const child = graph.state.nodesById.get(childId);
		if (child?.decl_id === declId) {
			return child;
		}
	}
	return null;
};

const frequencyValue = (value: ParamValue, fallback: number): number => {
	let frequency: number | null = null;
	if (value.kind === 'int' || value.kind === 'float') {
		frequency = Math.round(value.value);
	} else if (value.kind === 'str') {
		frequency = Number.parseInt(value.value.trim(), 10);
	}
	return frequency !== null && Number.isFinite(frequency) && frequency > 0 ? frequency : fallback;
};

export const engineLowFrequencyFromGraph = (graph: GraphStore): number => {
	const rootId = graph.state.rootId;
	if (rootId === null) {
		return DEFAULT_ENGINE_LOW_FREQUENCY_HZ;
	}
	const root = graph.state.nodesById.get(rootId);
	const preferences = childByDeclId(graph, root, PREFERENCES_DECL_ID);
	const engine = childByDeclId(graph, preferences, ENGINE_DECL_ID);
	const lowFrequency = childByDeclId(graph, engine, ENGINE_LOW_FREQUENCY_DECL_ID);
	if (!lowFrequency || lowFrequency.data.kind !== 'parameter') {
		return DEFAULT_ENGINE_LOW_FREQUENCY_HZ;
	}
	return frequencyValue(lowFrequency.data.param.value, DEFAULT_ENGINE_LOW_FREQUENCY_HZ);
};

export const showOscAddressInHelp = (graph: GraphStore): boolean => {
	const rootId = graph.state.rootId;
	const root = rootId === null ? null : graph.state.nodesById.get(rootId);
	const preferences = childByDeclId(graph, root, PREFERENCES_DECL_ID);
	const ui = childByDeclId(graph, preferences, INTERFACE_DECL_ID);
	const field = childByDeclId(graph, ui, SHOW_OSC_ADDRESS_IN_HELP_DECL_ID);
	return (
		field?.data.kind === 'parameter' &&
		field.data.param.value.kind === 'bool' &&
		field.data.param.value.value
	);
};

export const parameterControlStateEquals = (
	left: UiParameterControlState,
	right: UiParameterControlState
): boolean => left.mode === right.mode && JSON.stringify(left.spec) === JSON.stringify(right.spec);
