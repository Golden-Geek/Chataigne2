import type { NodeId, UiNodeDto } from 'golden_ui';
import { formulaANodes } from '../alchemistGraph';
import type {
	ANodeOutputPreviewSampleDto,
	ContextKeyDto,
	FormulaPreviewModeDto,
	RuntimeValueDto,
	StateMachineProtocolBundle
} from '../../state_machine/generated';

export const STATE_MACHINE_RUNTIME_PREVIEW_TOPIC = 'chataigne.state_machine.runtime_preview';

export interface FormulaOutputPreviewChip {
	label: string;
	title: string;
	status: ANodeOutputPreviewSampleDto['status'];
	logicalTick: number;
	value: RuntimeValueDto;
	active?: boolean;
}

const DEFAULT_LANE_ID = '__default__';

const contextKeyId = (contextKey: ContextKeyDto | null): string =>
	contextKey && contextKey.parts.length > 0
		? contextKey.parts.map((part) => `${part.axis_id}:${part.item_id}`).join('|')
		: DEFAULT_LANE_ID;

const compactNumber = (value: number): string => {
	if (!Number.isFinite(value)) return String(value);
	const formatted = Math.abs(value) >= 1000 ? value.toPrecision(4) : value.toFixed(3);
	return formatted.replace(/\.?0+($|e)/, '$1');
};

const compactInteger = (value: bigint | number): string => {
	const text = String(value);
	return text.length > 8 ? `${text.slice(0, 7)}...` : text;
};

const runtimeValueLabel = (value: RuntimeValueDto): string => {
	switch (value.kind) {
		case 'unit':
			return '';
		case 'bool':
			return value.value ? 'true' : 'false';
		case 'trigger':
			return value.fired ? 'fired' : 'idle';
		case 'int':
			return compactInteger(value.value);
		case 'float':
			return compactNumber(value.value);
		case 'string':
			return value.value.length > 18 ? `"${value.value.slice(0, 17)}..."` : `"${value.value}"`;
		case 'vec2':
			return `${compactNumber(value.value[0])}, ${compactNumber(value.value[1])}`;
		case 'vec3':
			return `${compactNumber(value.value[0])}, ${compactNumber(value.value[1])}, ${compactNumber(value.value[2])}`;
		case 'color':
			return `rgba ${compactNumber(value.red)}, ${compactNumber(value.green)}, ${compactNumber(value.blue)}, ${compactNumber(value.alpha)}`;
		case 'duration':
			return `${compactNumber(value.seconds)}s`;
		case 'array':
			return `[${value.values.length}]`;
		case 'ref':
			return value.value_type || 'ref';
		case 'extension':
			return value.value_type || 'extension';
	}
};

const runtimeValueTitle = (sample: ANodeOutputPreviewSampleDto): string => {
	const value = runtimeValueLabel(sample.value) || sample.value.kind;
	const lane = contextKeyId(sample.context_key);
	return `${sample.output_socket_id}: ${value} (${sample.status}, ${lane})`;
};

const logicalTick = (value: bigint | number): number => Number(value);

const newerSample = (
	current: FormulaOutputPreviewChip | undefined,
	sample: ANodeOutputPreviewSampleDto
): boolean => current === undefined || logicalTick(sample.logical_tick) >= current.logicalTick;

const sampleMatchesPreviewMode = (
	sample: ANodeOutputPreviewSampleDto,
	mode: FormulaPreviewModeDto
): boolean => {
	switch (mode.kind) {
		case 'formula_defaults':
			return sample.formula_id === mode.formula_id && sample.processor_id === null;
		case 'processor_default_lane':
			return (
				sample.processor_id === mode.processor_id &&
				contextKeyId(sample.context_key) === DEFAULT_LANE_ID
			);
		case 'processor_lane':
			return (
				sample.processor_id === mode.processor_id &&
				contextKeyId(sample.context_key) === contextKeyId(mode.context_key)
			);
	}
};

export const formulaOutputPreviewMap = (
	formula: UiNodeDto | null,
	nodesById: ReadonlyMap<NodeId, UiNodeDto>,
	bundle: StateMachineProtocolBundle | null,
	mode: FormulaPreviewModeDto | null
): ReadonlyMap<string, FormulaOutputPreviewChip> => {
	if (!formula || !bundle || !mode) return new Map();
	const anodeNodeIdByUuid = new Map(
		formulaANodes(formula, nodesById).map((anode) => [anode.uuid, anode.node_id])
	);
	const result = new Map<string, FormulaOutputPreviewChip>();
	for (const sample of bundle.output_preview) {
		if (sample.formula_id !== formula.uuid) continue;
		if (!sampleMatchesPreviewMode(sample, mode)) continue;
		const nodeId = anodeNodeIdByUuid.get(sample.node_id);
		if (nodeId === undefined) continue;
		const label = runtimeValueLabel(sample.value);
		const key = `${nodeId}:${sample.output_socket_id}`;
		const current = result.get(key);
		if (!newerSample(current, sample)) continue;
		result.set(key, {
			label,
			title: runtimeValueTitle(sample),
			status: sample.status,
			logicalTick: logicalTick(sample.logical_tick),
			value: sample.value
		});
	}
	return result;
};
