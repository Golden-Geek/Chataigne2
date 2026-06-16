import type { ContextKeyDto, FormulaPreviewModeDto, ProcessorLaneSummaryDto } from '../generated';
import type { UiNodeDto } from 'golden_ui';

export type FormulaPreviewEditLevel = 'formula_recipe' | 'processor_instance' | 'selected_lane';

export interface FormulaPreviewLaneOption {
	id: string;
	label: string;
	contextKey: ContextKeyDto | null;
	hasMemory: boolean;
	diagnosticsCount: number;
}

export interface FormulaPreviewSessionModel {
	formulaNodeId: number | null;
	processorNodeId: number | null;
	level: FormulaPreviewEditLevel;
	mode: FormulaPreviewModeDto | null;
	lanes: FormulaPreviewLaneOption[];
	selectedLaneId: string | null;
	title: string;
	subtitle: string;
}

const DEFAULT_LANE_ID = '__default__';

const contextKeyId = (contextKey: ContextKeyDto | null): string =>
	contextKey && contextKey.parts.length > 0
		? contextKey.parts.map((part) => `${part.axis_id}:${part.item_id}`).join('|')
		: DEFAULT_LANE_ID;

const contextKeyLabel = (contextKey: ContextKeyDto | null): string =>
	contextKey && contextKey.parts.length > 0
		? contextKey.parts.map((part) => part.item_label || part.item_id).join(' / ')
		: 'Default lane';

const laneOption = (lane: ProcessorLaneSummaryDto): FormulaPreviewLaneOption => ({
	id: contextKeyId(lane.context_key),
	label: lane.label || contextKeyLabel(lane.context_key),
	contextKey: lane.context_key,
	hasMemory: lane.has_memory,
	diagnosticsCount: lane.diagnostics_count
});

const previewSubtitle = (
	processor: UiNodeDto | null,
	selectedLane: FormulaPreviewLaneOption | null
): string => {
	if (processor === null) return 'Formula defaults';
	const laneLabel = selectedLane?.contextKey ? selectedLane.label : 'Default lane';
	return `Processor instance: ${processor.meta.label} / ${laneLabel}`;
};

class FormulaPreviewSessionStore {
	private selectedLaneByProcessor = $state<Record<string, string>>({});

	selectLane(processorNodeId: number | null, laneId: string): void {
		if (processorNodeId === null) return;
		this.selectedLaneByProcessor = {
			...this.selectedLaneByProcessor,
			[String(processorNodeId)]: laneId
		};
	}

	model(
		formula: UiNodeDto | null,
		processor: UiNodeDto | null,
		laneSummaries: readonly ProcessorLaneSummaryDto[]
	): FormulaPreviewSessionModel {
		const lanes =
			processor === null
				? []
				: laneSummaries.length > 0
					? laneSummaries.map(laneOption)
					: [
							{
								id: DEFAULT_LANE_ID,
								label: 'Default lane',
								contextKey: null,
								hasMemory: false,
								diagnosticsCount: 0
							}
						];
		const selectedLaneId =
			processor === null
				? null
				: (this.selectedLaneByProcessor[String(processor.node_id)] ??
					lanes[0]?.id ??
					DEFAULT_LANE_ID);
		const selectedLane = lanes.find((lane) => lane.id === selectedLaneId) ?? lanes[0] ?? null;
		const level: FormulaPreviewEditLevel =
			processor === null
				? 'formula_recipe'
				: selectedLane?.contextKey
					? 'selected_lane'
					: 'processor_instance';
		const mode: FormulaPreviewModeDto | null =
			formula === null
				? null
				: processor === null
					? { kind: 'formula_defaults', formula_id: formula.uuid }
					: selectedLane?.contextKey
						? {
								kind: 'processor_lane',
								processor_id: processor.uuid,
								context_key: selectedLane.contextKey
							}
						: { kind: 'processor_default_lane', processor_id: processor.uuid };

		return {
			formulaNodeId: formula?.node_id ?? null,
			processorNodeId: processor?.node_id ?? null,
			level,
			mode,
			lanes,
			selectedLaneId: selectedLane?.id ?? null,
			title: formula ? `Watching ${formula.meta.label}` : 'No Formula',
			subtitle: previewSubtitle(processor, selectedLane)
		};
	}
}

export const formulaPreviewSessionStore = new FormulaPreviewSessionStore();
