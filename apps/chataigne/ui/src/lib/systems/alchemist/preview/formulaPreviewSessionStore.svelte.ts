import type {
	ContextKeyDto,
	FormulaPreviewModeDto,
	ProcessorLaneCatalogEntryDto
} from '../../state_machine/generated';
import type { UiNodeDto } from 'golden_ui';

export type FormulaPreviewEditLevel = 'formula_recipe' | 'processor_instance' | 'selected_lane';

export const FOLLOW_PROCESSOR_LANE_ID = '__follow_processor__';

export interface FormulaPreviewLaneOption {
	id: string;
	label: string;
	contextKey: ContextKeyDto | null;
	hasMemory: boolean;
}

export interface FormulaPreviewSessionModel {
	formulaNodeId: number | null;
	processorNodeId: number | null;
	level: FormulaPreviewEditLevel;
	mode: FormulaPreviewModeDto | null;
	lanes: FormulaPreviewLaneOption[];
	selectedLaneId: string | null;
	laneSelectionId: string | null;
	processorLaneLabel: string | null;
	title: string;
	subtitle: string;
}

export const DEFAULT_LANE_ID = '__default__';

export const contextKeyId = (contextKey: ContextKeyDto | null): string =>
	contextKey && contextKey.parts.length > 0
		? contextKey.parts.map((part) => `${part.axis_id}:${part.item_id}`).join('|')
		: DEFAULT_LANE_ID;

const contextKeyLabel = (contextKey: ContextKeyDto | null): string =>
	contextKey && contextKey.parts.length > 0
		? contextKey.parts.map((part) => part.item_label || part.item_id).join(' / ')
		: 'Default lane';

const laneOption = (lane: ProcessorLaneCatalogEntryDto): FormulaPreviewLaneOption => ({
	id: contextKeyId(lane.context_key),
	label: lane.label || contextKeyLabel(lane.context_key),
	contextKey: lane.context_key,
	hasMemory: lane.has_memory
});

export const processorPreviewLaneOptions = (
	laneCatalog: readonly ProcessorLaneCatalogEntryDto[]
): FormulaPreviewLaneOption[] => laneCatalog.map(laneOption);

const previewSubtitle = (
	processor: UiNodeDto | null,
	selectedLane: FormulaPreviewLaneOption | null
): string => {
	if (processor === null) return 'Formula defaults';
	const laneLabel = selectedLane?.contextKey ? selectedLane.label : 'Default lane';
	return `Processor instance: ${processor.meta.label} / ${laneLabel}`;
};

class FormulaPreviewSessionStore {
	private processorLaneByProcessor = $state<Record<string, string>>({});
	private editorLaneByProcessor = $state<Record<string, string>>({});

	selectProcessorLane(processorNodeId: number | null, laneId: string): void {
		if (processorNodeId === null) return;
		this.processorLaneByProcessor = {
			...this.processorLaneByProcessor,
			[String(processorNodeId)]: laneId
		};
	}

	selectEditorLane(processorNodeId: number | null, laneId: string): void {
		if (processorNodeId === null) return;
		this.editorLaneByProcessor = {
			...this.editorLaneByProcessor,
			[String(processorNodeId)]: laneId
		};
	}

	processorLane(
		processorNodeId: number | null,
		lanes: readonly FormulaPreviewLaneOption[]
	): FormulaPreviewLaneOption | null {
		if (processorNodeId === null) return null;
		const selectedId = this.processorLaneByProcessor[String(processorNodeId)];
		return lanes.find((lane) => lane.id === selectedId) ?? lanes[0] ?? null;
	}

	model(
		formula: UiNodeDto | null,
		processor: UiNodeDto | null,
		laneCatalog: readonly ProcessorLaneCatalogEntryDto[]
	): FormulaPreviewSessionModel {
		const lanes =
			processor === null
				? []
				: laneCatalog.length > 0
					? processorPreviewLaneOptions(laneCatalog)
					: [
							{
								id: DEFAULT_LANE_ID,
								label: 'Default lane',
								contextKey: null,
								hasMemory: false,
							}
						];
		const processorLane = this.processorLane(processor?.node_id ?? null, lanes);
		const laneSelectionId =
			processor === null
				? null
				: (this.editorLaneByProcessor[String(processor.node_id)] ?? FOLLOW_PROCESSOR_LANE_ID);
		const selectedLane =
			laneSelectionId === FOLLOW_PROCESSOR_LANE_ID
				? processorLane
				: (lanes.find((lane) => lane.id === laneSelectionId) ?? processorLane);
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
			laneSelectionId,
			processorLaneLabel: processorLane?.label ?? null,
			title: formula ? `${formula.meta.label}` : 'No Formula',
			subtitle: previewSubtitle(processor, selectedLane)
		};
	}
}

export const formulaPreviewSessionStore = new FormulaPreviewSessionStore();
