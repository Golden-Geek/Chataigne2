import type { UiNodeDto, WorkbenchSession } from 'golden_ui';
import type {
	FormulaPreviewDemandDto,
	FormulaPreviewModeDto,
	ProcessorLaneCatalogEntryDto,
	ProcessorUiDto,
	RuntimeValueDto,
	StateMachinePreviewCatalogDto,
	StateMachineRuntimePreviewDto
} from '../../state_machine/generated';
import {
	STATE_MACHINE_RUNTIME_PREVIEW_DEMAND_TOPIC,
	STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC,
	STATE_MACHINE_RUNTIME_PREVIEW_TOPIC,
	formulaOutputPreviewMap,
	type FormulaOutputPreviewChip
} from '../preview/formulaOutputPreviewStore.svelte';
import { formulaPreviewSessionStore } from '../preview/formulaPreviewSessionStore.svelte';

export function createFormulaEditorPreview(
	source: () => {
		session: WorkbenchSession | null;
		formula: UiNodeDto | null;
		processorNode: UiNodeDto | null;
		manager: UiNodeDto | null;
	}
) {
	let session = $derived(source().session);
	let graphState = $derived(session?.graph.state ?? null);
	let formula = $derived(source().formula);
	let processorNode = $derived(source().processorNode);
	let stateMachineManager = $derived(source().manager);
	let outputPreviews = $state(new Map<string, FormulaOutputPreviewChip>());
	let activeSocketRefs = $state(new Set<string>());
	let retainedPreviewScopeKey = $state('');
	let lastMergedPreviewSequence = $state<number | null>(null);
	let previewActivityTimeout: ReturnType<typeof setTimeout> | null = null;
	let previewActivityDeadlines = new Map<string, number>();
	const PREVIEW_ACTIVITY_HOLD_MS = 160;
	const previewDemandSubscriptionId = `alchemist-preview:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`;
	const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());
	const previewModeKey = (mode: FormulaPreviewModeDto | null): string => {
		if (!mode) return '';
		switch (mode.kind) {
			case 'formula_defaults':
				return `${mode.kind}:${mode.formula_id}`;
			case 'processor_default_lane':
				return `${mode.kind}:${mode.processor_id}`;
			case 'processor_lane':
				return `${mode.kind}:${mode.processor_id}:${mode.context_key.parts
					.map((part) => `${part.axis_id}:${part.item_id}`)
					.join('|')}`;
		}
		return '';
	};

	const runtimeValueSignature = (value: RuntimeValueDto): string => {
		switch (value.kind) {
			case 'unit':
				return 'unit';
			case 'bool':
				return `bool:${value.value}`;
			case 'trigger':
				return `trigger:${value.fired}:${value.edge_id}:${value.logical_tick}`;
			case 'int':
				return `int:${value.value}`;
			case 'float':
				return `float:${value.value}`;
			case 'string':
				return `string:${value.value}`;
			case 'vec2':
			case 'vec3':
				return `${value.kind}:${value.value.join(':')}`;
			case 'color':
				return `color:${value.red}:${value.green}:${value.blue}:${value.alpha}`;
			case 'duration':
				return `duration:${value.seconds}`;
			case 'array':
				return `array:${value.values.map(runtimeValueSignature).join('|')}`;
			case 'ref':
				return `ref:${value.value_type}:${value.stable_id}`;
			case 'extension':
				return `extension:${value.value_type}:${value.payload.join(':')}`;
		}
	};

	const previewActivitySignature = (preview: FormulaOutputPreviewChip): string =>
		`${preview.logicalTick}:${runtimeValueSignature(preview.value)}`;

	const publishActiveSocketRefs = (): void => {
		activeSocketRefs = new Set(previewActivityDeadlines.keys());
	};

	const cancelPreviewActivityTimeout = (): void => {
		if (previewActivityTimeout !== null) {
			clearTimeout(previewActivityTimeout);
			previewActivityTimeout = null;
		}
	};

	const prunePreviewActivity = (): void => {
		const currentTime = nowMs();
		let changed = false;
		for (const [ref, deadline] of previewActivityDeadlines) {
			if (deadline > currentTime) continue;
			previewActivityDeadlines.delete(ref);
			changed = true;
		}
		if (changed) {
			publishActiveSocketRefs();
		}
		schedulePreviewActivityPrune();
	};

	const schedulePreviewActivityPrune = (): void => {
		cancelPreviewActivityTimeout();
		if (previewActivityDeadlines.size === 0) return;
		const nextDeadline = Math.min(...previewActivityDeadlines.values());
		previewActivityTimeout = setTimeout(prunePreviewActivity, Math.max(0, nextDeadline - nowMs()));
	};

	const clearPreviewActivity = (): void => {
		cancelPreviewActivityTimeout();
		previewActivityDeadlines = new Map();
		activeSocketRefs = new Set();
	};

	const resetRetainedPreviewState = (): void => {
		outputPreviews = new Map();
		lastMergedPreviewSequence = null;
		clearPreviewActivity();
	};

	const latchPreviewActivity = (refs: Iterable<string>): void => {
		const deadline = nowMs() + PREVIEW_ACTIVITY_HOLD_MS;
		let changed = false;
		for (const ref of refs) {
			if ((previewActivityDeadlines.get(ref) ?? 0) >= deadline) continue;
			previewActivityDeadlines.set(ref, deadline);
			changed = true;
		}
		if (changed) {
			publishActiveSocketRefs();
			schedulePreviewActivityPrune();
		}
	};

	let runtimePreviewSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_TOPIC) ?? 0
	);
	let runtimePreview = $derived.by((): StateMachineRuntimePreviewDto | null => {
		runtimePreviewSequence;
		return (
			session?.getCustomEventPayload<StateMachineRuntimePreviewDto>(
				STATE_MACHINE_RUNTIME_PREVIEW_TOPIC
			) ?? null
		);
	});
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
	let runtimeProcessorLaneCatalog = $derived.by((): ProcessorLaneCatalogEntryDto[] => {
		if (!processorNode || !runtimePreviewCatalog) return [];
		return runtimePreviewCatalog.processor_lanes.filter(
			(lane) => lane.processor_id === processorNode.uuid
		);
	});
	let processorUi = $derived.by((): ProcessorUiDto | null => {
		if (!processorNode || !runtimePreviewCatalog) return null;
		return (
			runtimePreviewCatalog.processors.find((processor) => processor.id === processorNode.uuid) ??
			null
		);
	});
	let processorLaneCatalog = $derived(runtimeProcessorLaneCatalog);
	let previewSessionModel = $derived(
		formulaPreviewSessionStore.model(formula, processorNode, processorLaneCatalog)
	);

	const publishPreviewDemand = (mode: FormulaPreviewModeDto | null): void => {
		const activeSession = session;
		const manager = stateMachineManager;
		if (!activeSession || activeSession.status !== 'connected' || !manager) return;
		const payload: FormulaPreviewDemandDto = {
			subscription_id: previewDemandSubscriptionId,
			mode
		};
		void activeSession
			.sendIntent({
				kind: 'sendNodeEvent',
				node: manager.node_id,
				topic: STATE_MACHINE_RUNTIME_PREVIEW_DEMAND_TOPIC,
				payload
			})
			.catch(() => undefined);
	};

	$effect(() => {
		void session?.graphTransitionRevision;
		publishPreviewDemand(previewSessionModel.mode);
	});
	let incomingOutputPreviews = $derived.by(() =>
		formulaOutputPreviewMap(
			formula,
			graphState?.nodesById ?? new Map(),
			runtimePreview,
			previewSessionModel.mode
		)
	);
	$effect(() => () => clearPreviewActivity());
	$effect(() => {
		const previewScopeKey =
			formula && previewSessionModel.mode
				? `${formula.uuid}:${previewModeKey(previewSessionModel.mode)}`
				: '';
		const scopeChanged = previewScopeKey !== retainedPreviewScopeKey;
		if (previewScopeKey !== retainedPreviewScopeKey) {
			retainedPreviewScopeKey = previewScopeKey;
			resetRetainedPreviewState();
		}
		if (!previewScopeKey) return;
		const sequence = runtimePreviewSequence;
		if (lastMergedPreviewSequence === sequence) return;
		lastMergedPreviewSequence = sequence;

		const previous = outputPreviews;
		const next = new Map(incomingOutputPreviews);
		const updatedRefs: string[] = [];
		for (const [ref, preview] of next) {
			const current = previous.get(ref);
			const previewChanged =
				!current || previewActivitySignature(current) !== previewActivitySignature(preview);
			if (!previewChanged) continue;
			if (preview.value.kind === 'trigger' && !preview.value.fired) continue;
			updatedRefs.push(ref);
		}
		outputPreviews = next;
		if (scopeChanged) return;
		if (updatedRefs.length === 0) return;
		latchPreviewActivity(updatedRefs);
	});

	return {
		get outputPreviews() {
			return outputPreviews;
		},
		get activeSocketRefs() {
			return activeSocketRefs;
		},
		get processorUi() {
			return processorUi;
		},
		get model() {
			return previewSessionModel;
		},
		publishPreviewDemand
	};
}
