<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		GraphCamera,
		GraphConnectionRequest,
		GraphNodeCreationRequest,
		GraphNodeMove,
		GraphNodePosition,
		GraphNodeResize
	} from 'golden_graph_ui';
	import type {
		ContextMenuAnchor,
		ContextMenuItem,
		NodeId,
		PanelProps,
		PanelState,
		ParamValue,
		UiCreateUserItemInitialParam,
		UiCreatableUserItem,
		UiDuplicateCreateUserItemSpec,
		UiDuplicateDependentUserItem,
		UiDuplicateNodeSpec,
		UiEditIntent,
		UiNodeDto
	} from 'golden_ui';
	import {
		ContextMenu,
		ManagerListPanel,
		NodeAddButton,
		buildCreatableItemMenu,
		canDragOutlinerNode
	} from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { copyTextToClipboard } from 'golden_ui/utils/clipboard';
	import { sendCreateUserItemByTypeIntent } from 'golden_ui/store/ui-intents';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		ANODE_CREATE_PREFIX,
		ANODE_NODE_TYPE,
		CONNECTION_NODE_TYPE,
		FORMULA_NODE_TYPE,
		PROPERTIES_DECL_ID,
		PROPERTY_FOLDER_NODE_TYPE,
		PROPERTY_MANAGER_NODE_TYPE,
		PROPERTY_NODE_TYPE,
		anodeCategoryColor,
		anodeDefaultColor,
		directChild,
		formulaANodes,
		canConnectGraphConnection,
		parameterChild,
		toGraphEdges,
		toGraphNodes
	} from '../alchemistGraph';
	import type {
		FormulaPreviewDemandDto,
		FormulaPreviewModeDto,
		ManagedItemDto,
		ManagedRegionDefinitionDto,
		ManagedRegionInstanceDto,
		ProcessorLaneCatalogEntryDto,
		ProcessorUiDto,
		RuntimeValueDto,
		StateMachinePreviewCatalogDto,
		StateMachineRuntimePreviewDto
	} from '../../state_machine/generated';
	import {
		alchemistClipboardFromJson,
		alchemistClipboardJson,
		buildAlchemistClipboard,
		findEmptyAlchemistDuplicateOffset,
		nextAvailableAlchemistLabel,
		type AlchemistClipboard,
		type AlchemistClipboardNode,
		type AlchemistClipboardTreeNode
	} from '../alchemistClipboard';
	import {
		STATE_MACHINE_RUNTIME_PREVIEW_DEMAND_TOPIC,
		STATE_MACHINE_RUNTIME_PREVIEW_CATALOG_TOPIC,
		STATE_MACHINE_RUNTIME_PREVIEW_TOPIC,
		formulaOutputPreviewMap,
		type FormulaOutputPreviewChip
	} from '../preview/formulaOutputPreviewStore.svelte';
	import { formulaPreviewSessionStore } from '../preview/formulaPreviewSessionStore.svelte';
	import {
		formulaIsExternalFile,
		formulaIsReadOnly,
		formulaSourceDisplay as getFormulaSourceDisplay,
		formulaSourceKind
	} from '../formulaSource';
	import AlchemistGraphEditor from './AlchemistGraphEditor.svelte';
	import FormulaPreviewModeSelector from './FormulaPreviewModeSelector.svelte';
	import GraphToolbarActions from './GraphToolbarActions.svelte';
	import ProcessorLaneSelector from './ProcessorLaneSelector.svelte';

	interface ClipboardReferenceLookup {
		bySourceId: Map<NodeId, UiNodeDto>;
		bySourceUuid: Map<string, UiNodeDto>;
	}

	const DIAGNOSTICS_DECL_ID = 'diagnostics_json';
	const VALID_DECL_ID = 'is_valid';

	interface FormulaDiagnostic {
		code: string;
		message: string;
		severity: 'info' | 'warning' | 'error';
		origin: string;
	}

	interface PreviewTarget {
		kind: 'formula' | 'processor';
		nodeId: number;
	}

	interface SaveFilePickerOptions {
		suggestedName?: string;
		types?: Array<{
			description: string;
			accept: Record<string, string[]>;
		}>;
	}

	interface FileSystemWritableFileStream {
		write: (data: BlobPart) => Promise<void>;
		close: () => Promise<void>;
	}

	interface FileSystemFileHandle {
		createWritable: () => Promise<FileSystemWritableFileStream>;
	}

	type SaveFilePickerWindow = Window & {
		showSaveFilePicker?: (options?: SaveFilePickerOptions) => Promise<FileSystemFileHandle>;
	};

	interface AlchemistEditorPanelPersistedState {
		autoWire?: boolean;
	}

	const PROPERTY_DRAG_TYPE = 'application/x-chataigne-alchemist-property';

	const MIN_PANEL_WIDTH = 160;
	const MAX_PANEL_WIDTH = 520;
	const DEFAULT_PANEL_WIDTH = 240;
	const FORMULA_CAMERA_STORAGE_PREFIX = 'chataigne.alchemist.formula_camera:';
	const HIDDEN_ANODE_CREATE_TYPES = new Set([`${ANODE_CREATE_PREFIX}property`]);
	const MANAGER_ANODE_BY_ROLE: Record<string, string> = {
		condition: `${ANODE_CREATE_PREFIX}chataigne.conditions_manager`,
		filter: `${ANODE_CREATE_PREFIX}chataigne.filters_manager`,
		input: `${ANODE_CREATE_PREFIX}chataigne.inputs_manager`,
		output: `${ANODE_CREATE_PREFIX}chataigne.outputs_manager`
	};
	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PROCESSOR_MANAGED_REGIONS_DECL_ID = 'managed_regions';
	const PROCESSOR_MANAGED_REGION_DECL_PREFIX = 'managed_region/';
	const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';
	const FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX = 'chataigne.formula.external.builtin:';
	const FORMULA_COPY_SOURCE_DECL_ID = 'formula_copy_source';
	const CONDITION_GATE_CREATE_TYPE = `${ANODE_CREATE_PREFIX}condition_gate`;
	const PREVIEW_ACTIVITY_HOLD_MS = 160;
	const PREVIEW_DEMAND_HEARTBEAT_MS = 2000;
	const STATE_MACHINE_MANAGER_NODE_TYPE = 'state_machine_manager';

	let props: PanelProps = $props();
	let updatedPanelState = $state<PanelState | null>(null);
	let panelState = $derived(
		updatedPanelState ?? {
			panelId: props.panelId,
			panelType: props.panelType,
			title: props.title,
			params: props.params
		}
	);
	let panelRoot: HTMLElement | null = $state(null);
	let graphEditor: {
		clientToWorld: (clientX: number, clientY: number) => GraphNodePosition;
		frameSelection: () => boolean;
		home: () => boolean;
		focus: () => void;
		viewportCenter: () => GraphNodePosition;
	} | null = $state(null);
	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let saveStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
	let propertiesVisible = $state(true);
	let propertiesWidth = $state(DEFAULT_PANEL_WIDTH);
	let formulaCameras = $state<Record<string, GraphCamera>>({});
	let autoWire = $state(true);
	let initializedAutoWirePanelId: string | null = null;
	let previewTarget = $state<PreviewTarget | null>(null);
	let outputPreviews = $state(new Map<string, FormulaOutputPreviewChip>());
	let activeSocketRefs = $state(new Set<string>());
	let retainedPreviewScopeKey = $state('');
	let lastMergedPreviewSequence = $state<number | null>(null);
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuWorldPosition: GraphNodePosition | null = null;
	let alchemistClipboard = $state<AlchemistClipboard | null>(null);
	let alchemistDuplicateInProgress = false;
	let persistenceTail = Promise.resolve();
	let previewActivityTimeout: ReturnType<typeof setTimeout> | null = null;
	let previewActivityDeadlines = new Map<string, number>();
	const previewDemandSubscriptionId = `alchemist-preview:${Date.now().toString(36)}:${Math.random().toString(36).slice(2, 10)}`;

	const nowMs = (): number => (typeof performance !== 'undefined' ? performance.now() : Date.now());

	$effect(() => {
		if (initializedAutoWirePanelId === panelState.panelId) {
			return;
		}
		autoWire =
			readPanelPersistedState<AlchemistEditorPanelPersistedState>(panelState.params).autoWire !==
			false;
		initializedAutoWirePanelId = panelState.panelId;
	});

	const setAutoWire = (enabled: boolean): void => {
		autoWire = enabled;
		writePanelPersistedState(props.panelApi, { autoWire });
	};

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

	const isFormula = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.node_type === FORMULA_NODE_TYPE;

	const isProcessor = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.user_item_kind === PROCESSOR_ITEM_KIND || node?.node_type === PROCESSOR_ITEM_KIND;

	const formulaCameraStorageKey = (formulaUuid: string): string =>
		`${FORMULA_CAMERA_STORAGE_PREFIX}${formulaUuid}`;

	const cameraFromStorage = (formulaUuid: string): GraphCamera | undefined => {
		if (typeof localStorage === 'undefined') return undefined;
		const raw = localStorage.getItem(formulaCameraStorageKey(formulaUuid));
		if (!raw) return undefined;
		try {
			const parsed = JSON.parse(raw) as Partial<GraphCamera>;
			const { x, y, zoom } = parsed;
			if (
				typeof x === 'number' &&
				Number.isFinite(x) &&
				typeof y === 'number' &&
				Number.isFinite(y) &&
				typeof zoom === 'number' &&
				Number.isFinite(zoom) &&
				zoom > 0
			) {
				return { x, y, zoom };
			}
		} catch {
			return undefined;
		}
		return undefined;
	};

	const persistFormulaCamera = (formulaUuid: string, camera: GraphCamera): void => {
		formulaCameras = { ...formulaCameras, [formulaUuid]: camera };
		if (typeof localStorage === 'undefined') return;
		localStorage.setItem(formulaCameraStorageKey(formulaUuid), JSON.stringify(camera));
	};

	let requestedFormulaNodeId = $derived.by(() => {
		const value = panelState.params.formulaNodeId;
		const parsed = typeof value === 'number' ? value : Number(value);
		return Number.isInteger(parsed) ? parsed : null;
	});
	let requestedProcessorNodeId = $derived.by(() => {
		const value = panelState.params.processorNodeId;
		const parsed = typeof value === 'number' ? value : Number(value);
		return Number.isInteger(parsed) ? parsed : null;
	});
	let formulaNodes = $derived.by((): UiNodeDto[] => {
		if (!graphState) return [];
		return [...graphState.nodesById.values()]
			.filter(isFormula)
			.sort((left, right) => left.meta.label.localeCompare(right.meta.label));
	});
	let formulaLibrary = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		return (
			[...graphState.nodesById.values()].find(
				(node) => node.node_type === FORMULA_LIBRARY_NODE_TYPE
			) ?? null
		);
	});
	let stateMachineManager = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		const root = graphState.rootId === null ? null : graphState.nodesById.get(graphState.rootId);
		if (!root) return null;
		return (
			root.children
				.map((nodeId) => graphState.nodesById.get(nodeId))
				.find((node) => node?.node_type === STATE_MACHINE_MANAGER_NODE_TYPE) ?? null
		);
	});

	let selectedFormula = $derived.by((): UiNodeDto | null => {
		if (!session || !graphState) return null;
		for (const selectedId of session.selectedNodesIds) {
			const selected = graphState.nodesById.get(selectedId);
			if (isFormula(selected)) return selected;
		}
		return null;
	});

	let selectedProcessor = $derived.by((): UiNodeDto | null => {
		if (!session || !graphState) return null;
		for (const selectedId of session.selectedNodesIds) {
			const selected = graphState.nodesById.get(selectedId);
			if (isProcessor(selected)) return selected;
		}
		return null;
	});

	let requestedFormula = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		if (requestedFormulaNodeId === null) return null;
		const requested = graphState.nodesById.get(requestedFormulaNodeId);
		return isFormula(requested) ? requested : null;
	});

	let requestedProcessor = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		if (requestedProcessorNodeId === null) return null;
		const requested = graphState.nodesById.get(requestedProcessorNodeId);
		return isProcessor(requested) ? requested : null;
	});

	const setPreviewTarget = (next: PreviewTarget | null): void => {
		if (previewTarget?.kind === next?.kind && previewTarget?.nodeId === next?.nodeId) return;
		previewTarget = next;
	};

	const previewTargetNode = (target: PreviewTarget | null): UiNodeDto | null => {
		if (!graphState || target === null) return null;
		const node = graphState.nodesById.get(target.nodeId);
		if (target.kind === 'processor') return isProcessor(node) ? node : null;
		return isFormula(node) ? node : null;
	};

	$effect(() => {
		if (selectedProcessor) {
			setPreviewTarget({ kind: 'processor', nodeId: selectedProcessor.node_id });
			return;
		}
		if (selectedFormula) {
			setPreviewTarget({ kind: 'formula', nodeId: selectedFormula.node_id });
			return;
		}
		if (previewTargetNode(previewTarget) !== null) return;
		if (requestedProcessor) {
			setPreviewTarget({ kind: 'processor', nodeId: requestedProcessor.node_id });
			return;
		}
		if (requestedFormula) {
			setPreviewTarget({ kind: 'formula', nodeId: requestedFormula.node_id });
			return;
		}
		const firstFormula = formulaNodes[0] ?? null;
		setPreviewTarget(firstFormula ? { kind: 'formula', nodeId: firstFormula.node_id } : null);
	});

	let processorNode = $derived.by((): UiNodeDto | null => {
		if (previewTarget?.kind !== 'processor') return null;
		return previewTargetNode(previewTarget);
	});

	const formulaForProcessor = (processor: UiNodeDto | null): UiNodeDto | null => {
		if (!graphState || !processor) return null;
		const formulaParam = parameterChild(processor, graphState.nodesById, 'formula');
		if (
			formulaParam?.data.kind !== 'parameter' ||
			formulaParam.data.param.value.kind !== 'reference'
		) {
			return null;
		}
		const reference = formulaParam.data.param.value;
		if (reference.cached_id !== undefined) {
			const cached = graphState.nodesById.get(reference.cached_id);
			if (isFormula(cached) && cached.uuid === reference.uuid) return cached;
		}
		for (const node of graphState.nodesById.values()) {
			if (isFormula(node) && node.uuid === reference.uuid) return node;
		}
		return null;
	};

	let formula = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		const processorFormula = formulaForProcessor(processorNode);
		if (processorNode) return processorFormula;
		if (previewTarget?.kind === 'formula') return previewTargetNode(previewTarget);
		return requestedFormula ?? formulaNodes[0] ?? null;
	});
	let formulaSource = $derived(formulaSourceKind(formula, graphState?.nodesById ?? new Map()));
	let formulaSourceDisplay = $derived(getFormulaSourceDisplay(formulaSource));
	let formulaExternalFile = $derived(formulaIsExternalFile(formula));
	let formulaReadOnly = $derived(formulaIsReadOnly(formula));
	let formulaSourceBadgeLabel = $derived(
		formulaSource === 'project' && formulaExternalFile
			? 'External'
			: formulaSource === 'project' && formulaReadOnly
				? 'Read-only'
				: formulaSourceDisplay.badgeLabel
	);
	let formulaSourceBadgeTitle = $derived(
		formulaSource === 'project' && formulaExternalFile
			? 'Project formula linked to an external file'
			: formulaSource === 'project' && formulaReadOnly
				? 'Read-only project formula'
				: formulaSourceDisplay.title
	);
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
	let processorRegionInstances = $derived.by((): Map<string, ManagedRegionInstanceDto> => {
		return new Map(
			processorUi?.managed_region_instances.map((instance) => [instance.region_id, instance]) ?? []
		);
	});
	let processorManagedRegionsRoot = $derived.by((): UiNodeDto | null => {
		if (!processorNode || !graphState) return null;
		return directChild(processorNode, graphState.nodesById, PROCESSOR_MANAGED_REGIONS_DECL_ID);
	});
	let processorManagedRegionNodes = $derived.by((): Map<string, UiNodeDto> => {
		const nodes = new Map<string, UiNodeDto>();
		if (!processorManagedRegionsRoot || !graphState) return nodes;
		for (const childId of processorManagedRegionsRoot.children) {
			const child = graphState.nodesById.get(childId);
			if (!child?.decl_id.startsWith(PROCESSOR_MANAGED_REGION_DECL_PREFIX)) continue;
			nodes.set(child.decl_id.slice(PROCESSOR_MANAGED_REGION_DECL_PREFIX.length), child);
		}
		return nodes;
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
	let formulaCamera = $derived.by((): GraphCamera | undefined => {
		if (!formula) return undefined;
		return formulaCameras[formula.uuid] ?? cameraFromStorage(formula.uuid);
	});

	let diagnosticsParameter = $derived(
		graphState ? parameterChild(formula, graphState.nodesById, DIAGNOSTICS_DECL_ID) : null
	);
	let validParameter = $derived(
		graphState ? parameterChild(formula, graphState.nodesById, VALID_DECL_ID) : null
	);
	let formulaValid = $derived(
		validParameter?.data.kind === 'parameter' && validParameter.data.param.value.kind === 'bool'
			? validParameter.data.param.value.value
			: false
	);
	let diagnostics = $derived.by((): FormulaDiagnostic[] => {
		if (
			diagnosticsParameter?.data.kind !== 'parameter' ||
			diagnosticsParameter.data.param.value.kind !== 'str'
		) {
			return [];
		}
		try {
			const parsed: unknown = JSON.parse(diagnosticsParameter.data.param.value.value);
			return Array.isArray(parsed) ? (parsed as FormulaDiagnostic[]) : [];
		} catch {
			return [];
		}
	});
	let primaryDiagnostic = $derived(
		diagnostics.find((diagnostic) => diagnostic.severity === 'error') ?? diagnostics[0] ?? null
	);
	let formulaStatusTitle = $derived(
		formulaValid ? 'Formula valid' : (primaryDiagnostic?.message ?? 'Formula invalid')
	);
	const managedRegionKindLabel = (region: ManagedRegionDefinitionDto): string => {
		switch (region.kind) {
			case 'input_set':
				return 'Inputs';
			case 'filter_pipeline':
				return 'Filters';
			case 'output_set':
				return 'Outputs';
			case 'trigger_input':
				return 'Trigger';
			case 'command_set':
				return 'Commands';
		}
	};
	const managedRegionItemState = (item: ManagedItemDto): string =>
		item.enabled && item.anode_enabled ? item.anode_type_id : `${item.anode_type_id} off`;
	let anodeItems = $derived(
		formulaReadOnly
			? []
			: (formula?.creatable_user_items
					.filter(
						(item) =>
							item.node_type.startsWith(ANODE_CREATE_PREFIX) &&
							!HIDDEN_ANODE_CREATE_TYPES.has(item.node_type)
					)
					.map((item) => {
						const typeId = item.node_type.slice(ANODE_CREATE_PREFIX.length);
						const category = item.menu_path[0] ?? '';
						return {
							...item,
							menu_path_colors: item.menu_path.map((segment, index) =>
								index === 0 ? anodeCategoryColor(segment) : anodeDefaultColor(category, typeId)
							),
							color: anodeDefaultColor(category, typeId)
						};
					}) ?? [])
	);
	let properties = $derived(
		graphState ? directChild(formula, graphState.nodesById, PROPERTIES_DECL_ID) : null
	);
	let activePropertyContainer = $derived.by((): UiNodeDto | null => {
		if (!graphState || !properties) return properties ?? null;
		for (const selectedId of session?.selectedNodesIds ?? []) {
			const selected = graphState.nodesById.get(selectedId);
			if (!selected || selected.creatable_user_items.length === 0) continue;
			let currentId: number | undefined = selectedId;
			while (currentId !== undefined) {
				if (currentId === properties.node_id) return selected;
				currentId = graphState.parentById.get(currentId);
			}
		}
		return properties;
	});
	let anodeNodeIds = $derived(
		new Set(
			formula && graphState
				? formulaANodes(formula, graphState.nodesById).map((node) => node.node_id)
				: []
		)
	);
	let connectionNodeIds = $derived(
		new Set(
			formula && graphState
				? toGraphEdges(formula, graphState.nodesById).flatMap((edge) => {
						const nodeId = Number(edge.id);
						return Number.isSafeInteger(nodeId) ? [nodeId] : [];
					})
				: []
		)
	);
	let selectedNodeIds = $derived(
		(session?.selectedNodesIds ?? []).filter((nodeId) => anodeNodeIds.has(nodeId)).map(String)
	);
	let selectedEdgeIds = $derived(
		(session?.selectedNodesIds ?? []).filter((nodeId) => connectionNodeIds.has(nodeId)).map(String)
	);
	let contextMenuAnchor = $derived.by((): ContextMenuAnchor => ({
		kind: 'point',
		x: contextMenuX,
		y: contextMenuY
	}));

	$effect(() => {
		props.panelApi.setTitle(
			formula
				? `Alchemist: ${formula.meta.label}`
				: processorUi
					? `Alchemist: ${processorUi.formula_label}`
					: 'Alchemist Editor'
		);
	});

	const isPropertyTreeNode = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		Boolean(
			node &&
			(node.node_type === PROPERTY_NODE_TYPE ||
				node.node_type === PROPERTY_MANAGER_NODE_TYPE ||
				node.node_type === PROPERTY_FOLDER_NODE_TYPE)
		);

	const canRenderPropertyChildren = (node: UiNodeDto): boolean =>
		node.node_type === PROPERTY_MANAGER_NODE_TYPE || node.node_type === PROPERTY_FOLDER_NODE_TYPE;

	const canMovePropertyNode = (node: UiNodeDto): boolean =>
		isPropertyTreeNode(node) && canDragOutlinerNode(graphState, node);

	const setPropertyGraphDragData = (node: UiNodeDto, event: DragEvent): void => {
		if (!event.dataTransfer) return;
		event.dataTransfer.effectAllowed = 'copyMove';
		if (node.node_type === PROPERTY_NODE_TYPE || node.node_type === PROPERTY_MANAGER_NODE_TYPE) {
			event.dataTransfer.setData(PROPERTY_DRAG_TYPE, String(node.node_id));
		}
	};

	const initialParam = (
		decl_id: string,
		value: UiCreateUserItemInitialParam['value']
	): UiCreateUserItemInitialParam => ({ decl_id, value });

	const duplicateFormulaLabel = (label: string): string => {
		const used = new Set(formulaNodes.map((node) => node.meta.label));
		return nextAvailableAlchemistLabel(label, 'Formula', used);
	};

	const formulaReferenceValue = (target: UiNodeDto): UiCreateUserItemInitialParam['value'] => ({
		kind: 'reference',
		uuid: target.uuid,
		cached_id: target.node_id,
		cached_name: target.meta.label,
		relative_path_from_root: []
	});

	const formulaNodeForBuiltinSource = (sourceKey: string): UiNodeDto | null => {
		const sourceTag = `${FORMULA_EXTERNAL_BUILTIN_TAG_PREFIX}${sourceKey}`;
		return formulaNodes.find((formula) => formula.meta.tags.includes(sourceTag)) ?? null;
	};

	const runMutation = (operation: () => Promise<void>): Promise<void> => {
		saveStatus = 'saving';
		const queued = persistenceTail
			.catch(() => undefined)
			.then(operation)
			.then(() => {
				saveStatus = 'saved';
			})
			.catch((error: unknown) => {
				saveStatus = 'error';
				console.error('failed to edit Alchemist Formula', error);
				throw error;
			});
		persistenceTail = queued.catch(() => undefined);
		return queued;
	};

	const createNode = (
		item: UiCreatableUserItem,
		position: GraphNodePosition = graphEditor?.viewportCenter() ?? { x: 0, y: 0 }
	): void => {
		if (
			!formula ||
			formulaReadOnly ||
			!graphState ||
			!anodeItems.some((candidate) => candidate.node_type === item.node_type)
		)
			return;
		contextMenuOpen = false;
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				formula.node_id,
				item.node_type,
				item.label,
				{
					select_when_created: true,
					created_node_type: ANODE_NODE_TYPE,
					initial_params: [
						...item.initial_params,
						initialParam('position', {
							kind: 'vec2',
							value: [position.x, position.y]
						})
					]
				}
			);
			if (!result.success) throw new Error(`failed to create ${item.label}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const createPropertyItem = (parent: UiNodeDto, item: UiCreatableUserItem): void => {
		if (formulaReadOnly) return;
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				parent.node_id,
				item.node_type,
				item.label,
				{
					select_when_created: true,
					initial_params: item.initial_params
				}
			);
			if (!result.success) throw new Error(`failed to create ${item.label}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const createEditableBuiltInFormulaCopy = (): void => {
		if (
			!processorUi ||
			processorUi.formula_source_kind !== 'builtin' ||
			!processorUi.formula_can_duplicate_to_library ||
			!formulaLibrary ||
			!formulaLibrary.creatable_user_items.some((item) => item.node_type === FORMULA_NODE_TYPE)
		) {
			return;
		}
		const formulaLabel = processorUi.formula_label;
		const sourceKey = processorUi.formula_source_key;
		if (!sourceKey) return;
		const sourceFormula = formulaNodeForBuiltinSource(sourceKey);
		if (!sourceFormula) return;
		const libraryNodeId = formulaLibrary.node_id;
		const label = duplicateFormulaLabel(formulaLabel);
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(libraryNodeId, FORMULA_NODE_TYPE, label, {
				select_when_created: true,
				created_node_type: FORMULA_NODE_TYPE,
				initial_params: [
					initialParam(FORMULA_COPY_SOURCE_DECL_ID, formulaReferenceValue(sourceFormula))
				]
			});
			if (!result.success) throw new Error(`failed to duplicate ${formulaLabel}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
				setPreviewTarget({ kind: 'formula', nodeId: result.createdNodeId });
			}
		});
	};

	const createManagedRegionItem = (regionNode: UiNodeDto, item: UiCreatableUserItem): void => {
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				regionNode.node_id,
				item.node_type,
				item.label,
				{
					select_when_created: true,
					created_node_type: ANODE_NODE_TYPE,
					initial_params: item.initial_params
				}
			);
			if (!result.success) throw new Error(`failed to create ${item.label}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const managedRegionItems = (regionNode: UiNodeDto | null | undefined): UiCreatableUserItem[] =>
		regionNode?.creatable_user_items ?? [];

	const managedRegionConditionGate = (
		regionNode: UiNodeDto | null | undefined
	): UiCreatableUserItem | null =>
		managedRegionItems(regionNode).find((item) => item.node_type === CONDITION_GATE_CREATE_TYPE) ??
		null;

	const createPropertyGetter = (property: UiNodeDto, position: GraphNodePosition): void => {
		if (!formula || formulaReadOnly || !graphState || property.node_type !== PROPERTY_NODE_TYPE) {
			return;
		}
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				formula.node_id,
				`${ANODE_CREATE_PREFIX}property`,
				property.meta.label,
				{
					select_when_created: true,
					created_node_type: ANODE_NODE_TYPE,
					initial_params: [
						initialParam('position', {
							kind: 'vec2',
							value: [position.x, position.y]
						}),
						initialParam('config/property_id', {
							kind: 'reference',
							uuid: property.uuid,
							cached_id: property.node_id,
							cached_name: property.meta.label,
							relative_path_from_root: []
						})
					]
				}
			);
			if (!result.success) {
				throw new Error(`failed to create getter for ${property.meta.label}`);
			}
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const managerRole = (manager: UiNodeDto): string | null => {
		if (!graphState) return null;
		const role = parameterChild(manager, graphState.nodesById, 'role');
		if (role?.data.kind !== 'parameter') return null;
		const value = role.data.param.value;
		return value.kind === 'enum' ? value.value : null;
	};

	const createManagerGetter = (manager: UiNodeDto, position: GraphNodePosition): void => {
		if (
			!formula ||
			formulaReadOnly ||
			!graphState ||
			manager.node_type !== PROPERTY_MANAGER_NODE_TYPE
		) {
			return;
		}
		const role = managerRole(manager);
		const nodeType = role ? MANAGER_ANODE_BY_ROLE[role] : undefined;
		if (!nodeType) return;
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				formula.node_id,
				nodeType,
				manager.meta.label,
				{
					select_when_created: true,
					created_node_type: ANODE_NODE_TYPE,
					initial_params: [
						initialParam('position', {
							kind: 'vec2',
							value: [position.x, position.y]
						}),
						initialParam('config/manager_id', {
							kind: 'reference',
							uuid: manager.uuid,
							cached_id: manager.node_id,
							cached_name: manager.meta.label,
							relative_path_from_root: []
						})
					]
				}
			);
			if (!result.success) {
				throw new Error(`failed to create manager node for ${manager.meta.label}`);
			}
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const allowPropertyDrop = (event: DragEvent): void => {
		if (formulaReadOnly) return;
		if (!event.dataTransfer?.types.includes(PROPERTY_DRAG_TYPE)) return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'copy';
	};

	const dropProperty = (event: DragEvent): void => {
		if (formulaReadOnly || !graphState) return;
		const position = graphEditor?.clientToWorld(event.clientX, event.clientY) ?? { x: 0, y: 0 };

		const propertyId = Number(event.dataTransfer?.getData(PROPERTY_DRAG_TYPE));
		if (Number.isSafeInteger(propertyId) && propertyId !== 0) {
			const property = graphState.nodesById.get(propertyId);
			if (property?.node_type === PROPERTY_NODE_TYPE) {
				event.preventDefault();
				createPropertyGetter(property, position);
				return;
			}
			if (property?.node_type === PROPERTY_MANAGER_NODE_TYPE) {
				event.preventDefault();
				createManagerGetter(property, position);
				return;
			}
		}
	};

	let contextMenuItems = $derived.by((): ContextMenuItem[] => {
		const createItems = buildCreatableItemMenu(anodeItems, (item) =>
			createNode(item, contextMenuWorldPosition ?? graphEditor?.viewportCenter() ?? { x: 0, y: 0 })
		);
		const clipboard = selectedAlchemistClipboard();
		const editItems: ContextMenuItem[] = [];
		if (clipboard) {
			editItems.push(
				{
					id: 'copy-selection',
					label: 'Copy',
					commandId: 'edit.copy',
					action: () => {
						copySelectedGraphItems();
						contextMenuOpen = false;
					}
				},
				{
					id: 'export-selection-json',
					label: 'Export as JSON',
					action: () => {
						void exportAlchemistClipboardJson(clipboard);
						contextMenuOpen = false;
					}
				}
			);
		}
		if (!formulaReadOnly) {
			editItems.push({
				id: 'paste-alchemist-clipboard',
				label: 'Paste',
				commandId: 'edit.paste',
				action: () => {
					void pasteGraphItems();
					contextMenuOpen = false;
				}
			});
		}
		return createItems.length > 0 ? [...editItems, { separator: true }, ...createItems] : editItems;
	});

	const delay = (durationMs: number): Promise<void> =>
		new Promise((resolve) => setTimeout(resolve, durationMs));

	const nextAlchemistEditId = (prefix: string): string =>
		`${prefix}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;

	const waitForAlchemistEditIdle = async (operation: string): Promise<void> => {
		if (!session) throw new Error(`${operation} requires an active workbench session`);
		if (session.hasActiveEditSession) {
			await session.refreshSnapshot();
		}
		const deadline = Date.now() + 750;
		while (session.hasActiveEditSession && Date.now() <= deadline) {
			await delay(16);
		}
		if (session.hasActiveEditSession) {
			throw new Error(`${operation} is blocked by an active edit session`);
		}
	};

	const closeAlchemistEditSession = async (
		clientEditId: string,
		forceRefresh = false
	): Promise<void> => {
		if (!session) return;
		if (!session.hasActiveEditSession) {
			if (forceRefresh) {
				await session.refreshSnapshot();
			}
			return;
		}
		const endIntent: UiEditIntent = { kind: 'endEdit', client_edit_id: clientEditId };
		try {
			await session.sendIntents([endIntent]);
		} catch {
			await session.refreshSnapshot();
		}
		const deadline = Date.now() + 750;
		while (session.hasActiveEditSession && Date.now() <= deadline) {
			await delay(16);
		}
	};

	const sendAlchemistEditBatch = async (
		label: string,
		intents: UiEditIntent[],
		idPrefix = 'alchemist-edit'
	): Promise<void> => {
		if (intents.length === 0) return;
		if (!session) throw new Error(`${label} requires an active workbench session`);
		await waitForAlchemistEditIdle(label);
		const clientEditId = nextAlchemistEditId(idPrefix);
		try {
			await session.sendIntents([
				{ kind: 'beginEdit', client_edit_id: clientEditId, label },
				...intents,
				{ kind: 'endEdit', client_edit_id: clientEditId }
			]);
		} catch (error) {
			await closeAlchemistEditSession(clientEditId, true);
			throw error;
		}
	};

	const editParameters = async (label: string, intents: UiEditIntent[]): Promise<void> =>
		sendAlchemistEditBatch(label, intents, 'alchemist-formula');

	const selectedAlchemistClipboard = (): AlchemistClipboard | null => {
		if (!formula || formulaReadOnly || !graphState || !session) return null;
		return buildAlchemistClipboard({
			formula,
			nodesById: graphState.nodesById,
			selectedNodeIds: session.selectedNodesIds,
			anodeNodeIds,
			anodeItems
		});
	};

	const readAlchemistClipboardFromSystem = async (): Promise<AlchemistClipboard | null> => {
		if (typeof navigator === 'undefined' || !navigator.clipboard?.readText) return null;
		try {
			const text = await navigator.clipboard.readText();
			const clipboard = alchemistClipboardFromJson(text);
			if (!clipboard) return null;
			alchemistClipboard = clipboard;
			return clipboard;
		} catch (error) {
			console.warn('failed to read Alchemist clipboard text', error);
			return null;
		}
	};

	const alchemistClipboardFileName = (clipboard: AlchemistClipboard): string => {
		const label =
			clipboard.nodes.length === 1 ? clipboard.nodes[0].label.trim() : 'alchemist-nodes';
		const stem = (label || 'alchemist-node')
			.replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
			.replace(/\s+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 80);
		return `${stem || 'alchemist-node'}.json`;
	};

	const downloadTextFile = (fileName: string, text: string): void => {
		const blob = new Blob([text], { type: 'application/json;charset=utf-8' });
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = fileName;
		link.rel = 'noopener';
		document.body.appendChild(link);
		link.click();
		link.remove();
		URL.revokeObjectURL(url);
	};

	const saveTextFile = async (fileName: string, text: string): Promise<void> => {
		const picker = (window as SaveFilePickerWindow).showSaveFilePicker;
		if (!picker) {
			downloadTextFile(fileName, text);
			return;
		}
		const handle = await picker({
			suggestedName: fileName,
			types: [
				{
					description: 'JSON',
					accept: { 'application/json': ['.json'] }
				}
			]
		});
		const writable = await handle.createWritable();
		await writable.write(text);
		await writable.close();
	};

	const exportAlchemistClipboardJson = async (clipboard: AlchemistClipboard): Promise<boolean> => {
		try {
			await saveTextFile(alchemistClipboardFileName(clipboard), alchemistClipboardJson(clipboard));
			return true;
		} catch (error) {
			if (error instanceof DOMException && error.name === 'AbortError') return false;
			console.error('failed to export Alchemist clipboard JSON', error);
			return false;
		}
	};

	const waitForCreatedAnode = async (
		parentId: NodeId,
		knownChildren: Set<NodeId>
	): Promise<UiNodeDto | null> => {
		const deadline = Date.now() + 750;
		while (Date.now() <= deadline) {
			const parent = graphState?.nodesById.get(parentId);
			if (parent) {
				for (const childId of parent.children) {
					if (knownChildren.has(childId)) continue;
					const child = graphState?.nodesById.get(childId);
					if (child?.node_type === ANODE_NODE_TYPE) {
						return child;
					}
				}
			}
			await new Promise((resolve) => setTimeout(resolve, 16));
		}
		return null;
	};

	const createClipboardReferenceLookup = (): ClipboardReferenceLookup => ({
		bySourceId: new Map(),
		bySourceUuid: new Map()
	});

	const addClipboardReferenceTarget = (
		lookup: ClipboardReferenceLookup,
		sourceTree: AlchemistClipboardTreeNode,
		targetNode: UiNodeDto
	): void => {
		lookup.bySourceId.set(sourceTree.sourceId, targetNode);
		lookup.bySourceUuid.set(sourceTree.sourceUuid, targetNode);
	};

	const appendClipboardReferenceTargets = (
		sourceTree: AlchemistClipboardTreeNode,
		targetNode: UiNodeDto,
		lookup: ClipboardReferenceLookup
	): void => {
		if (!graphState) return;
		addClipboardReferenceTarget(lookup, sourceTree, targetNode);
		for (const sourceChild of sourceTree.children) {
			const targetChild = directChild(targetNode, graphState.nodesById, sourceChild.decl_id);
			if (targetChild) appendClipboardReferenceTargets(sourceChild, targetChild, lookup);
		}
	};

	const importedParamValue = (
		value: ParamValue,
		referenceLookup: ClipboardReferenceLookup
	): ParamValue | null => {
		if (!graphState || value.kind !== 'reference') return value;
		const remapped =
			(value.cached_id === undefined
				? undefined
				: referenceLookup.bySourceId.get(value.cached_id)) ??
			referenceLookup.bySourceUuid.get(value.uuid);
		if (remapped) {
			return {
				...value,
				uuid: remapped.uuid,
				cached_id: remapped.node_id,
				cached_name: remapped.meta.label
			};
		}
		for (const candidate of graphState.nodesById.values()) {
			if (candidate.uuid !== value.uuid) continue;
			return {
				...value,
				cached_id: candidate.node_id,
				cached_name: candidate.meta.label
			};
		}
		return null;
	};

	const appendClipboardTreeRestoreIntents = (
		sourceTree: AlchemistClipboardTreeNode,
		targetNode: UiNodeDto,
		intents: UiEditIntent[],
		referenceLookup: ClipboardReferenceLookup,
		path: readonly string[] = []
	): void => {
		if (!graphState) return;
		if (targetNode.meta.can_be_disabled && targetNode.meta.enabled !== sourceTree.meta.enabled) {
			intents.push({
				kind: 'patchMeta',
				node: targetNode.node_id,
				patch: { enabled: sourceTree.meta.enabled }
			});
		}
		const collapsed = sourceTree.meta.presentation?.collapsed;
		if (typeof collapsed === 'boolean' && targetNode.meta.presentation?.collapsed !== collapsed) {
			intents.push({
				kind: 'patchMeta',
				node: targetNode.node_id,
				patch: {
					presentation: {
						...(targetNode.meta.presentation ?? {}),
						collapsed
					}
				}
			});
		}
		for (const sourceChild of sourceTree.children) {
			const targetChild = directChild(targetNode, graphState.nodesById, sourceChild.decl_id);
			if (!targetChild) continue;
			const nextPath = [...path, sourceChild.decl_id];
			const isPlacementParam = nextPath.length === 1 && sourceChild.decl_id === 'position';
			if (
				!isPlacementParam &&
				sourceChild.data.kind === 'parameter' &&
				targetChild.data.kind === 'parameter' &&
				!targetChild.data.param.read_only
			) {
				const value = importedParamValue(sourceChild.data.param.value, referenceLookup);
				if (value && JSON.stringify(value) !== JSON.stringify(targetChild.data.param.value)) {
					intents.push({
						kind: 'setParam',
						node: targetChild.node_id,
						value,
						behaviour: targetChild.data.param.event_behaviour
					});
				}
			}
			appendClipboardTreeRestoreIntents(
				sourceChild,
				targetChild,
				intents,
				referenceLookup,
				nextPath
			);
		}
	};

	const restoreCreatedClipboardTrees = async (
		created: Array<{ nodeId: NodeId; entry: AlchemistClipboardNode }>
	): Promise<void> => {
		if (!graphState || created.length === 0) return;
		const referenceLookup = createClipboardReferenceLookup();
		for (const { nodeId, entry } of created) {
			if (!entry.tree) continue;
			const target = graphState.nodesById.get(nodeId);
			if (target) appendClipboardReferenceTargets(entry.tree, target, referenceLookup);
		}
		const intents: UiEditIntent[] = [];
		for (const { nodeId, entry } of created) {
			if (!entry.tree) continue;
			const target = graphState.nodesById.get(nodeId);
			if (!target) continue;
			appendClipboardTreeRestoreIntents(entry.tree, target, intents, referenceLookup);
		}
		if (intents.length === 0) return;
		await editParameters('Restore Pasted ANode Data', intents);
	};

	const duplicateClipboardIntoFormula = async (
		clipboard: AlchemistClipboard,
		label: string
	): Promise<boolean> => {
		if (!formula || !graphState || !session || clipboard.nodes.length === 0) return false;
		const preferSourcePosition = clipboard.formulaId === formula.node_id;
		const offset = findEmptyAlchemistDuplicateOffset({
			nodes: clipboard.nodes,
			formula,
			nodesById: graphState.nodesById,
			anodeItems,
			viewportCenter: graphEditor?.viewportCenter() ?? null,
			preferSourcePosition
		});
		const createdEntries: Array<{ entry: AlchemistClipboardNode }> = [];
		const duplicateNodes: UiDuplicateNodeSpec[] = [];
		const createdItems: UiDuplicateCreateUserItemSpec[] = [];
		let insertAfterNodeId: NodeId | undefined =
			preferSourcePosition && clipboard.nodes.length > 0
				? clipboard.nodes[clipboard.nodes.length - 1].sourceId
				: formula.children[formula.children.length - 1];

		for (const entry of clipboard.nodes) {
			const nextPosition = {
				x: entry.position.x + offset.x,
				y: entry.position.y + offset.y
			};
			const source = graphState.nodesById.get(entry.sourceId);
			const sourceMatchesClipboard =
				source !== undefined &&
				(entry.sourceUuid === undefined || source.uuid === entry.sourceUuid);
			if (
				source &&
				sourceMatchesClipboard &&
				source.node_type === ANODE_NODE_TYPE &&
				source.meta.user_permissions.can_remove_and_duplicate
			) {
				duplicateNodes.push({
					source: source.node_id,
					new_parent: formula.node_id,
					new_prev_sibling: insertAfterNodeId,
					initial_params: [
						initialParam('position', {
							kind: 'vec2',
							value: [nextPosition.x, nextPosition.y]
						})
					]
				});
			} else if (
				formula.creatable_user_items.some((item) => item.node_type === entry.createNodeType)
			) {
				createdItems.push({
					source: entry.sourceId,
					parent: formula.node_id,
					node_type: entry.createNodeType,
					label: entry.label.trim().length > 0 ? entry.label.trim() : entry.node_type,
					initial_params: [
						initialParam('position', {
							kind: 'vec2',
							value: [nextPosition.x, nextPosition.y]
						})
					]
				});
			} else {
				continue;
			}
			createdEntries.push({ entry });
			insertAfterNodeId = undefined;
		}

		if (duplicateNodes.length === 0 && createdItems.length === 0) return false;

		const knownChildren = new Set(formula.children);
		const copiedSourceIds = new Set<NodeId>([
			...duplicateNodes.map((entry) => entry.source),
			...createdItems.map((entry) => entry.source)
		]);
		const dependentItems: UiDuplicateDependentUserItem[] = clipboard.edges
			.filter(
				(edge) => copiedSourceIds.has(edge.sourceNodeId) && copiedSourceIds.has(edge.targetNodeId)
			)
			.map((edge) => ({
				parent: formula.node_id,
				node_type: CONNECTION_NODE_TYPE,
				label: 'Connection',
				initial_params: [
					{
						decl_id: 'source_node',
						value: { kind: 'duplicatedNodeReference', source: edge.sourceNodeId }
					},
					{
						decl_id: 'source_socket',
						value: { kind: 'literal', value: { kind: 'str', value: edge.sourceSocketId } }
					},
					{
						decl_id: 'target_node',
						value: { kind: 'duplicatedNodeReference', source: edge.targetNodeId }
					},
					{
						decl_id: 'target_socket',
						value: { kind: 'literal', value: { kind: 'str', value: edge.targetSocketId } }
					}
				]
			}));

		await sendAlchemistEditBatch(
			label,
			[
				{
					kind: 'duplicateNodes',
					nodes: duplicateNodes,
					created_items: createdItems,
					dependent_items: dependentItems
				}
			],
			'alchemist-duplicate'
		);

		const createdNodeIds: NodeId[] = [];
		const importedCreatedEntries: Array<{ nodeId: NodeId; entry: AlchemistClipboardNode }> = [];
		for (const createdEntry of createdEntries) {
			const created = await waitForCreatedAnode(formula.node_id, knownChildren);
			if (!created) continue;
			knownChildren.add(created.node_id);
			createdNodeIds.push(created.node_id);
			const source = graphState.nodesById.get(createdEntry.entry.sourceId);
			if (
				!source ||
				(createdEntry.entry.sourceUuid !== undefined &&
					source.uuid !== createdEntry.entry.sourceUuid)
			) {
				importedCreatedEntries.push({ nodeId: created.node_id, entry: createdEntry.entry });
			}
		}
		await restoreCreatedClipboardTrees(importedCreatedEntries);

		if (createdNodeIds.length > 0) {
			session.selectNodes(createdNodeIds, 'REPLACE');
		}
		return true;
	};

	const copySelectedGraphItems = (): boolean => {
		const clipboard = selectedAlchemistClipboard();
		if (!clipboard) return false;
		alchemistClipboard = clipboard;
		void copyTextToClipboard(alchemistClipboardJson(clipboard));
		return true;
	};

	const duplicateSelectedGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		if (alchemistDuplicateInProgress) return true;
		const clipboard = selectedAlchemistClipboard();
		if (!clipboard) return false;
		let duplicated = false;
		alchemistDuplicateInProgress = true;
		await runMutation(async () => {
			try {
				duplicated = await duplicateClipboardIntoFormula(
					clipboard,
					clipboard.nodes.length === 1
						? 'Duplicate ANode'
						: `Duplicate ${clipboard.nodes.length} ANodes`
				);
			} finally {
				alchemistDuplicateInProgress = false;
			}
		});
		return duplicated;
	};

	const pasteGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		if (alchemistDuplicateInProgress) return true;
		const clipboard = (await readAlchemistClipboardFromSystem()) ?? alchemistClipboard;
		if (!clipboard) return false;
		let pasted = false;
		alchemistDuplicateInProgress = true;
		await runMutation(async () => {
			try {
				pasted = await duplicateClipboardIntoFormula(
					clipboard,
					clipboard.nodes.length === 1 ? 'Paste ANode' : `Paste ${clipboard.nodes.length} ANodes`
				);
			} finally {
				alchemistDuplicateInProgress = false;
			}
		});
		return pasted;
	};

	const cutSelectedGraphItems = async (): Promise<boolean> => {
		if (formulaReadOnly) return false;
		if (!copySelectedGraphItems()) return false;
		return removeSelectedGraphItems();
	};

	const moveNodes = (moves: GraphNodeMove[]): Promise<void> =>
		runMutation(async () => {
			if (formulaReadOnly || !graphState || moves.length === 0) return;
			const intents = moves.flatMap((move): UiEditIntent[] => {
				const anode = graphState.nodesById.get(Number(move.nodeId));
				const position = parameterChild(anode, graphState.nodesById, 'position');
				if (anode?.node_type !== ANODE_NODE_TYPE || position?.data.kind !== 'parameter') return [];
				return [
					{
						kind: 'setParam',
						node: position.node_id,
						value: { kind: 'vec2', value: [move.position.x, move.position.y] },
						behaviour: position.data.param.event_behaviour
					}
				];
			});
			await editParameters(
				moves.length === 1 ? 'Move ANode' : `Move ${moves.length} ANodes`,
				intents
			);
		});

	const canRemoveNode = (nodeId: NodeId): boolean => {
		const node = graphState?.nodesById.get(nodeId);
		return node?.meta.user_permissions.can_remove_and_duplicate === true;
	};

	const removeSelectedGraphItems = async (): Promise<boolean> => {
		if (!formula || formulaReadOnly || !graphState || !session) return false;
		const selectedIds = new Set(session.selectedNodesIds);
		const selectedAnodeIds = [...selectedIds].filter(
			(nodeId) => anodeNodeIds.has(nodeId) && canRemoveNode(nodeId)
		);
		const selectedAnodeIdSet = new Set(selectedAnodeIds);
		const removeIds = new Set<NodeId>();

		for (const nodeId of selectedIds) {
			if ((anodeNodeIds.has(nodeId) || connectionNodeIds.has(nodeId)) && canRemoveNode(nodeId)) {
				removeIds.add(nodeId);
			}
		}

		if (selectedAnodeIdSet.size > 0) {
			for (const edge of toGraphEdges(formula, graphState.nodesById)) {
				const edgeNodeId = edge.id === undefined ? NaN : Number(edge.id);
				if (!Number.isSafeInteger(edgeNodeId) || !canRemoveNode(edgeNodeId)) continue;
				const sourceNodeId = Number(edge.from.nodeId);
				const targetNodeId = Number(edge.to.nodeId);
				if (selectedAnodeIdSet.has(sourceNodeId) || selectedAnodeIdSet.has(targetNodeId)) {
					removeIds.add(edgeNodeId);
				}
			}
		}

		if (removeIds.size === 0) return false;
		await runMutation(async () => {
			await editParameters('Delete Alchemist Graph Selection', [
				{ kind: 'removeNodes', nodes: [...removeIds] }
			]);
		});
		return true;
	};

	const resizeNode = (resize: GraphNodeResize): Promise<void> =>
		runMutation(async () => {
			if (formulaReadOnly || !graphState) return;
			const anode = graphState.nodesById.get(Number(resize.nodeId));
			const size = parameterChild(anode, graphState.nodesById, 'size');
			if (
				anode?.node_type !== ANODE_NODE_TYPE ||
				size?.data.kind !== 'parameter' ||
				size.data.param.read_only ||
				!size.meta.can_be_disabled
			) {
				throw new Error('ANode size parameter is unavailable');
			}
			await editParameters(
				resize.mode === 'custom' ? 'Resize ANode' : 'Auto-size ANode',
				resize.mode === 'custom'
					? [
							{
								kind: 'setParam',
								node: size.node_id,
								value: { kind: 'vec2', value: [resize.size.width, resize.size.height] },
								behaviour: size.data.param.event_behaviour
							},
							{
								kind: 'patchMeta',
								node: size.node_id,
								patch: { enabled: true }
							}
						]
					: [
							{
								kind: 'patchMeta',
								node: size.node_id,
								patch: { enabled: false }
							}
						]
			);
		});

	const renameNode = (nodeId: string, label: string): Promise<void> =>
		runMutation(async () => {
			if (formulaReadOnly || !graphState) return;
			const anode = graphState.nodesById.get(Number(nodeId));
			const nextLabel = label.trim();
			if (anode?.node_type !== ANODE_NODE_TYPE || nextLabel.length === 0) return;
			await editParameters(`Rename ${anode.meta.label}`, [
				{
					kind: 'patchMeta',
					node: anode.node_id,
					patch: { label: nextLabel }
				}
			]);
		});

	const setNodeCollapsed = (nodeId: string, collapsed: boolean): Promise<void> =>
		runMutation(async () => {
			if (formulaReadOnly || !graphState) return;
			const anode = graphState.nodesById.get(Number(nodeId));
			if (anode?.node_type !== ANODE_NODE_TYPE) return;
			await editParameters(`${collapsed ? 'Collapse' : 'Expand'} ${anode.meta.label}`, [
				{
					kind: 'patchMeta',
					node: anode.node_id,
					patch: {
						presentation: {
							...(anode.meta.presentation ?? {}),
							collapsed
						}
					}
				}
			]);
		});

	const setNodeEnabled = (nodeId: string, enabled: boolean): Promise<void> =>
		runMutation(async () => {
			if (formulaReadOnly || !graphState) return;
			const anode = graphState.nodesById.get(Number(nodeId));
			if (anode?.node_type !== ANODE_NODE_TYPE || !anode.meta.can_be_disabled) return;
			await editParameters(`${enabled ? 'Enable' : 'Disable'} ${anode.meta.label}`, [
				{
					kind: 'patchMeta',
					node: anode.node_id,
					patch: { enabled }
				}
			]);
		});

	const setFormulaCamera = (camera: GraphCamera): void => {
		if (!formula) return;
		persistFormulaCamera(formula.uuid, camera);
	};

	const socketRootId = (socketId: string): string => {
		const dotIndex = socketId.lastIndexOf('.');
		if (dotIndex < 0) return socketId;
		const component = socketId.slice(dotIndex + 1);
		return ['x', 'y', 'z', 'r', 'g', 'b', 'a'].includes(component)
			? socketId.slice(0, dotIndex)
			: socketId;
	};

	const targetInputConflict = (edgeSocketId: string, nextSocketId: string): boolean => {
		const edgeRoot = socketRootId(edgeSocketId);
		const nextRoot = socketRootId(nextSocketId);
		if (edgeRoot !== nextRoot) return false;
		const edgeIsComponent = edgeSocketId !== edgeRoot;
		const nextIsComponent = nextSocketId !== nextRoot;
		return !edgeIsComponent || !nextIsComponent || edgeSocketId === nextSocketId;
	};

	const connectNodes = (connection: GraphConnectionRequest): void => {
		if (!formula || formulaReadOnly || !graphState) return;
		const source = graphState.nodesById.get(Number(connection.from.nodeId));
		const target = graphState.nodesById.get(Number(connection.to.nodeId));
		if (source?.node_type !== ANODE_NODE_TYPE || target?.node_type !== ANODE_NODE_TYPE) return;
		const graphNodes = toGraphNodes(formula, graphState.nodesById, anodeItems);
		if (!canConnectGraphConnection(graphNodes, connection)) return;
		const edges = toGraphEdges(formula, graphState.nodesById);
		if (
			edges.some(
				(edge) =>
					edge.from.nodeId === connection.from.nodeId &&
					edge.from.socketId === connection.from.socketId &&
					edge.to.nodeId === connection.to.nodeId &&
					edge.to.socketId === connection.to.socketId
			)
		) {
			return;
		}
		const replacedEdges = edges
			.filter(
				(edge) =>
					edge.to.nodeId === connection.to.nodeId &&
					targetInputConflict(edge.to.socketId, connection.to.socketId)
			)
			.flatMap((edge) => (edge.id === undefined ? [] : [Number(edge.id)]));
		void runMutation(async () => {
			const intents: UiEditIntent[] = [];
			if (replacedEdges.length > 0) intents.push({ kind: 'removeNodes', nodes: replacedEdges });
			intents.push({
				kind: 'createUserItem',
				parent: formula.node_id,
				node_type: CONNECTION_NODE_TYPE,
				label: 'Connection',
				initial_params: [
					initialParam('source_node', {
						kind: 'reference',
						uuid: source.uuid,
						cached_id: source.node_id,
						cached_name: source.meta.label,
						relative_path_from_root: []
					}),
					initialParam('source_socket', { kind: 'str', value: connection.from.socketId }),
					initialParam('target_node', {
						kind: 'reference',
						uuid: target.uuid,
						cached_id: target.node_id,
						cached_name: target.meta.label,
						relative_path_from_root: []
					}),
					initialParam('target_socket', { kind: 'str', value: connection.to.socketId })
				]
			});
			await editParameters('Connect ANodes', intents);
		});
	};

	const selectGraphItems = (nodeIds: string[], edgeIds: string[]): void => {
		if (!session || !formula) return;
		const hierarchyNodeIds = [...nodeIds, ...edgeIds]
			.map(Number)
			.filter(
				(nodeId) =>
					Number.isSafeInteger(nodeId) &&
					(anodeNodeIds.has(nodeId) || connectionNodeIds.has(nodeId))
			);
		session.selectNodes(
			hierarchyNodeIds.length > 0 ? hierarchyNodeIds : [formula.node_id],
			'REPLACE'
		);
	};

	const showCreateMenu = (clientX: number, clientY: number, position: GraphNodePosition): void => {
		if (contextMenuItems.length === 0) return;
		contextMenuX = clientX;
		contextMenuY = clientY;
		contextMenuWorldPosition = position;
		contextMenuOpen = true;
	};

	const openContextMenu = (event: MouseEvent, position: GraphNodePosition): void => {
		event.preventDefault();
		event.stopPropagation();
		showCreateMenu(event.clientX, event.clientY, position);
		graphEditor?.focus();
	};

	const openCreateRequest = (request: GraphNodeCreationRequest): void => {
		showCreateMenu(request.clientX, request.clientY, request.position);
	};

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	onMount(() => {
		const previewDemandHeartbeat = setInterval(
			() => publishPreviewDemand(previewSessionModel.mode),
			PREVIEW_DEMAND_HEARTBEAT_MS
		);
		const panelOwnsFocus = (): boolean =>
			panelRoot !== null &&
			document.activeElement !== null &&
			panelRoot.contains(document.activeElement);
		const unregisterFrame = registerCommandHandler(
			'view.frame',
			() => (panelOwnsFocus() ? (graphEditor?.frameSelection() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterHome = registerCommandHandler(
			'view.home',
			() => (panelOwnsFocus() ? (graphEditor?.home() ?? false) : false),
			{ priority: 100 }
		);
		const unregisterSelectAll = registerCommandHandler(
			'select.all',
			() => {
				if (!panelOwnsFocus() || !session) return false;
				const graphItems = [...anodeNodeIds, ...connectionNodeIds];
				if (graphItems.length === 0) return false;
				session.selectNodes(graphItems, 'REPLACE');
				return true;
			},
			{ priority: 100 }
		);
		const unregisterDeleteSelection = registerCommandHandler(
			'edit.deleteSelection',
			() => (panelOwnsFocus() ? removeSelectedGraphItems() : false),
			{ priority: 100 }
		);
		const unregisterCopy = registerCommandHandler(
			'edit.copy',
			() => (panelOwnsFocus() ? copySelectedGraphItems() : false),
			{ priority: 100 }
		);
		const unregisterCut = registerCommandHandler(
			'edit.cut',
			() => (panelOwnsFocus() ? cutSelectedGraphItems() : false),
			{ priority: 100 }
		);
		const unregisterDuplicate = registerCommandHandler(
			'edit.duplicate',
			() => (panelOwnsFocus() ? duplicateSelectedGraphItems() : false),
			{ priority: 100 }
		);
		const unregisterPaste = registerCommandHandler(
			'edit.paste',
			() => (panelOwnsFocus() ? pasteGraphItems() : false),
			{ priority: 100 }
		);
		return () => {
			clearInterval(previewDemandHeartbeat);
			publishPreviewDemand(null);
			unregisterFrame();
			unregisterHome();
			unregisterSelectAll();
			unregisterDeleteSelection();
			unregisterCopy();
			unregisterCut();
			unregisterDuplicate();
			unregisterPaste();
		};
	});
</script>

{#snippet graphToolbarContent()}
	{#if formula}
		<GraphToolbarActions
			{autoWire}
			onAutoWireChange={setAutoWire}
			addNode={formula}
			addItems={anodeItems}
			onCreateItem={(item) => createNode(item)} />
	{/if}
{/snippet}

<section bind:this={panelRoot} class="alchemist-editor-panel" aria-label={panelState.title}>
	<div class="editor-content">
		{#if graphState && (formula || processorUi)}
			<!-- Slide-in properties panel -->
			<aside
				class="properties-panel"
				class:visible={propertiesVisible}
				style:width="{propertiesWidth}px"
				aria-label="Formula properties">
				<div class="properties-heading">
					<button
						type="button"
						class="properties-toggle-btn"
						aria-pressed={propertiesVisible}
						title="Hide properties"
						onclick={() => (propertiesVisible = false)}>
						<span class="properties-toggle-chevron">‹</span>
						<span class="properties-toggle-label">{processorUi ? 'Processor' : 'Properties'}</span>
					</button>
				</div>
				<div class="properties-body">
					{#if processorUi && processorUi.managed_regions.length > 0}
						<div class="processor-surface" aria-label="Processor regions">
							<header class="processor-surface-header">
								<div class="processor-surface-title">
									<strong>{processorUi.label}</strong>
									{#if processorUi.formula_source_kind === 'builtin'}
										<span class="processor-source-pill">Built-in</span>
									{/if}
								</div>
								<div class="processor-surface-actions">
									<span class:off={!processorUi.active}
										>{processorUi.active ? 'Active' : 'Off'}</span>
									{#if processorUi.formula_source_kind === 'builtin' && processorUi.formula_open_readonly_from_processor}
										<span>Read-only</span>
									{/if}
									{#if processorUi.formula_source_kind === 'builtin' && processorUi.formula_can_duplicate_to_library}
										<button
											type="button"
											class="processor-formula-control-btn"
											disabled={!formulaLibrary}
											title="Create editable formula copy"
											onclick={createEditableBuiltInFormulaCopy}>
											Create Copy
										</button>
									{/if}
								</div>
							</header>
							{#each processorUi.managed_regions as region (region.id)}
								{@const instance = processorRegionInstances.get(region.id)}
								{@const regionNode = processorManagedRegionNodes.get(region.id) ?? null}
								{@const regionItems = managedRegionItems(regionNode)}
								{@const conditionGate = managedRegionConditionGate(regionNode)}
								<section class="processor-region">
									<header class="processor-region-header">
										<div class="processor-region-title">
											<strong>{region.label || managedRegionKindLabel(region)}</strong>
											<span>{managedRegionKindLabel(region)}</span>
										</div>
										<div class="processor-region-actions">
											{#if regionNode && conditionGate}
												<button
													type="button"
													class="processor-region-condition-btn"
													title="Add ConditionGate"
													onclick={() => createManagedRegionItem(regionNode, conditionGate)}>
													Condition
												</button>
											{/if}
											{#if regionNode && regionItems.length > 0}
												<NodeAddButton
													node={regionNode}
													items={regionItems}
													onCreateItem={(item) => createManagedRegionItem(regionNode, item)} />
											{/if}
										</div>
									</header>
									{#if instance && instance.items.length > 0}
										<ol class="processor-region-items">
											{#each instance.items as item (item.id)}
												<li class:off={!item.enabled || !item.anode_enabled}>
													<span>{item.label}</span>
													<small>{managedRegionItemState(item)}</small>
												</li>
											{/each}
										</ol>
									{:else}
										<p class="processor-region-empty">Empty</p>
									{/if}
								</section>
							{/each}
						</div>
					{/if}
					{#if formula}
						<ManagerListPanel
							managerNode={properties}
							addTargetNode={formulaReadOnly ? null : activePropertyContainer}
							addItems={formulaReadOnly ? [] : activePropertyContainer?.creatable_user_items}
							searchPlaceholder="Search properties..."
							missingMessage="Formula properties are not available."
							emptyMessage={formulaReadOnly
								? 'Built-in formula properties are read-only.'
								: 'Drag a property onto the graph to create a getter.'}
							rootDropMessage={formulaReadOnly
								? 'Built-in formula properties are read-only.'
								: 'Drop here to move into Properties.'}
							addButtonTitle="Add property item"
							isTreeNode={isPropertyTreeNode}
							canRenderNodeChildren={canRenderPropertyChildren}
							nodeDraggable={formulaReadOnly ? () => false : canMovePropertyNode}
							onNodeDragStartData={formulaReadOnly ? undefined : setPropertyGraphDragData}
							onCreateItem={formulaReadOnly
								? undefined
								: (parent, item) => createPropertyItem(parent, item)} />
					{/if}
				</div>
				<!-- Resize handle on right edge -->
				<div
					class="panel-resize-handle"
					role="separator"
					aria-label="Resize properties panel"
					aria-orientation="vertical"
					onpointerdown={(e) => {
						e.preventDefault();
						const startX = e.clientX;
						const startWidth = propertiesWidth;
						const el = e.currentTarget as HTMLElement;
						el.setPointerCapture(e.pointerId);
						el.onpointermove = (ev) => {
							propertiesWidth = Math.max(
								MIN_PANEL_WIDTH,
								Math.min(MAX_PANEL_WIDTH, startWidth + ev.clientX - startX)
							);
						};
						el.onpointerup = el.onpointercancel = (ev) => {
							el.releasePointerCapture(ev.pointerId);
							el.onpointermove = null;
							el.onpointerup = null;
							el.onpointercancel = null;
						};
					}}>
				</div>
			</aside>

			<!-- Collapsed-state tab — visible only when panel is hidden -->
			<button
				type="button"
				class="properties-show-tab"
				class:panel-visible={propertiesVisible}
				aria-hidden={propertiesVisible}
				title="Show properties"
				onclick={() => (propertiesVisible = true)}>
				<span class="properties-toggle-chevron">›</span>
				<span class="properties-toggle-label">Properties</span>
			</button>

			{#if formula}
				<div
					class="graph-drop-zone"
					role="application"
					aria-label="Alchemist graph drop target"
					ondragover={formulaReadOnly ? undefined : allowPropertyDrop}
					ondrop={formulaReadOnly ? undefined : dropProperty}>
					<AlchemistGraphEditor
						bind:this={graphEditor}
						{formula}
						nodesById={graphState.nodesById}
						{selectedNodeIds}
						{selectedEdgeIds}
						{outputPreviews}
						{activeSocketRefs}
						readOnly={formulaReadOnly}
						catalogItems={anodeItems}
						onGraphSelectionChange={selectGraphItems}
						onNodesMove={formulaReadOnly ? undefined : moveNodes}
						onNodeResize={formulaReadOnly ? undefined : resizeNode}
						onNodeRename={formulaReadOnly ? undefined : renameNode}
						onNodeCollapsedChange={formulaReadOnly ? undefined : setNodeCollapsed}
						onNodeEnabledChange={formulaReadOnly ? undefined : setNodeEnabled}
						onConnect={formulaReadOnly ? undefined : connectNodes}
						onBackgroundContextMenu={formulaReadOnly ? undefined : openContextMenu}
						onCreateRequest={formulaReadOnly ? undefined : openCreateRequest}
						initialCamera={formulaCamera}
						onCameraChange={setFormulaCamera}
						viewportInset={{ left: propertiesVisible ? propertiesWidth : 0 }}
						{autoWire}
						toolbarEnd={graphToolbarContent} />
					<div class="preview-status-bar" aria-label="Formula preview context">
						<span
							class="formula-source-pill"
							title={formulaSourceBadgeTitle}
							style:--formula-source-color={formulaSourceDisplay.accent}>
							{formulaSourceBadgeLabel}
						</span>
						<span
							class="formula-status"
							class:valid={formulaValid}
							class:error={!formulaValid}
							title={formulaStatusTitle}
							aria-label={formulaStatusTitle}>
							{formulaValid ? '✓' : '!'}
						</span>
						{#if saveStatus === 'saving' || saveStatus === 'error'}
							<span class="save-indicator" class:error={saveStatus === 'error'} aria-live="polite">
								{saveStatus === 'saving' ? '…' : '!'}
							</span>
						{/if}
						<FormulaPreviewModeSelector model={previewSessionModel} />
						<ProcessorLaneSelector
							lanes={previewSessionModel.lanes}
							selectedLaneId={previewSessionModel.laneSelectionId}
							followProcessorLabel={previewSessionModel.processorLaneLabel}
							onSelect={(laneId) =>
								formulaPreviewSessionStore.selectEditorLane(
									processorNode?.node_id ?? null,
									laneId
								)} />
					</div>
					{#if diagnostics.length > 0}
						<aside class="diagnostics" aria-label="Formula diagnostics">
							{#each diagnostics as diagnostic (`${diagnostic.code}:${diagnostic.origin}`)}
								<div class:error={diagnostic.severity === 'error'} class="diagnostic">
									<strong>{diagnostic.code}</strong>
									<span>{diagnostic.message}</span>
								</div>
							{/each}
						</aside>
					{/if}
				</div>
			{:else if processorUi && processorUi.formula_source_kind === 'builtin'}
				<div
					class="builtin-formula-view"
					style:padding-left={propertiesVisible ? `${propertiesWidth}px` : '0'}>
					<div class="builtin-formula-panel">
						<strong>{processorUi.formula_label}</strong>
						<span>Built-in, read-only</span>
						{#if processorUi.formula_can_duplicate_to_library}
							<button
								type="button"
								class="processor-formula-control-btn"
								disabled={!formulaLibrary}
								onclick={createEditableBuiltInFormulaCopy}>
								Create Editable Copy
							</button>
						{/if}
					</div>
				</div>
			{/if}
		{:else}
			<div class="empty-state">
				<strong>No Alchemist Formula</strong>
				<p>Create a Formula in the Formula Library to start authoring.</p>
			</div>
		{/if}
	</div>

	<ContextMenu
		bind:open={contextMenuOpen}
		items={contextMenuItems}
		anchor={contextMenuAnchor}
		minWidthRem={10}
		maxWidthCss="max-content" />
</section>

<style>
	.alchemist-editor-panel {
		position: relative;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
	}

	.editor-content {
		position: relative;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	/* Properties panel — glass overlay sliding in from the left */
	.properties-panel {
		position: absolute;
		inset-block: 0;
		inset-inline-start: 0;
		z-index: 30;
		display: grid;
		grid-template-rows: auto minmax(0, 1fr) auto;
		min-inline-size: 0;
		min-block-size: 0;
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		box-shadow: 0.5rem 0 2rem color-mix(in srgb, black 32%, transparent);
		transform: translateX(-100%);
		transition: transform 0.22s cubic-bezier(0.2, 0, 0.13, 1);
		pointer-events: none;
	}

	.properties-panel.visible {
		transform: translateX(0);
		pointer-events: auto;
	}

	/* Toggle button inside the panel heading */
	.properties-toggle-btn {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.18rem 0.3rem 0.18rem 0.2rem;
		border: none;
		border-radius: 0.3rem;
		background: transparent;
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition: background 0.12s;
	}

	.properties-toggle-btn:hover {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 14%, transparent);
	}

	.properties-toggle-chevron {
		font-size: 0.9rem;
		line-height: 1;
		color: color-mix(in srgb, var(--gc-color-text) 65%, transparent);
	}

	.properties-toggle-label {
		font-size: 0.74rem;
		font-weight: 600;
	}

	/* Collapsed-state tab — shown when panel is hidden */
	.properties-show-tab {
		position: absolute;
		top: 0;
		left: 0;
		z-index: 35;
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		padding: 0.35rem 0.55rem 0.35rem 0.4rem;
		border: none;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-inline-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		border-end-end-radius: 0.4rem;
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 72%, transparent);
		backdrop-filter: blur(14px);
		-webkit-backdrop-filter: blur(14px);
		color: var(--gc-color-text);
		font: inherit;
		cursor: pointer;
		transition:
			opacity 0.18s,
			background 0.12s;
	}

	.properties-show-tab.panel-visible {
		opacity: 0;
		pointer-events: none;
	}

	.properties-show-tab:not(.panel-visible):hover {
		background: color-mix(
			in srgb,
			var(--gc-color-accent, #66a6ff) 18%,
			var(--gc-color-background-soft, #1a1a1a)
		);
	}

	/* Resize handle on the right edge of the panel */
	.panel-resize-handle {
		position: absolute;
		inset-block: 0;
		inset-inline-end: 0;
		inline-size: 0.35rem;
		cursor: col-resize;
		z-index: 5;
		touch-action: none;
	}

	.panel-resize-handle:hover,
	.panel-resize-handle:active {
		background: color-mix(in srgb, var(--gc-color-accent, #66a6ff) 55%, transparent);
	}

	.properties-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.28rem 0.5rem 0.28rem 0.35rem;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
	}

	.properties-body {
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.processor-surface {
		display: grid;
		gap: 0.35rem;
		max-block-size: 42vh;
		padding: 0.45rem;
		overflow: auto;
		border-block-end: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 55%, transparent);
		background: color-mix(in srgb, var(--gc-color-background) 72%, transparent);
	}

	.processor-surface-header,
	.processor-region-header,
	.processor-region-items li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		min-inline-size: 0;
	}

	.processor-surface-header {
		font-size: 0.72rem;
	}

	.processor-surface-title,
	.processor-surface-actions {
		display: inline-flex;
		align-items: center;
		gap: 0.35rem;
		min-inline-size: 0;
	}

	.processor-surface-title {
		flex: 1 1 auto;
	}

	.processor-surface-actions {
		flex: 0 0 auto;
	}

	.processor-source-pill,
	.formula-source-pill {
		padding: 0.08rem 0.28rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 75%, transparent);
		border-radius: 999rem;
		background: color-mix(in srgb, var(--gc-color-accent, #5d8cff) 12%, transparent);
	}

	.formula-source-pill {
		display: inline-flex;
		align-items: center;
		min-block-size: 1rem;
		border-color: color-mix(in srgb, var(--formula-source-color) 48%, var(--gc-color-border));
		background: color-mix(in srgb, var(--formula-source-color) 16%, transparent);
		color: color-mix(in srgb, var(--formula-source-color) 62%, var(--gc-color-text));
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
	}

	.processor-surface-header strong,
	.processor-region-header strong,
	.processor-region-items span {
		min-inline-size: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.processor-surface-header span,
	.processor-region-header span,
	.processor-region-items small {
		flex: 0 0 auto;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.62rem;
	}

	.processor-surface-header span.off,
	.processor-region-items li.off {
		color: color-mix(in srgb, var(--gc-color-text) 42%, transparent);
	}

	.processor-region {
		display: grid;
		gap: 0.25rem;
		padding: 0.4rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 70%, transparent);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-background-soft, #1a1a1a) 68%, transparent);
	}

	.processor-region-header {
		font-size: 0.68rem;
	}

	.processor-region-title {
		display: grid;
		min-inline-size: 0;
	}

	.processor-region-actions {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 0.28rem;
	}

	.processor-region-condition-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-block-size: 1.45rem;
		max-inline-size: 7.5rem;
		padding: 0 0.45rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 78%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--gc-color-background) 82%, transparent);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
		cursor: pointer;
	}

	.processor-formula-control-btn {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		min-block-size: 1.45rem;
		max-inline-size: 11rem;
		padding: 0 0.5rem;
		border: 0.06rem solid color-mix(in srgb, var(--gc-color-border) 78%, transparent);
		border-radius: 0.25rem;
		background: color-mix(in srgb, var(--gc-color-background) 82%, transparent);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.62rem;
		line-height: 1;
		white-space: nowrap;
		cursor: pointer;
	}

	.processor-region-condition-btn:hover,
	.processor-region-condition-btn:focus-visible,
	.processor-formula-control-btn:hover,
	.processor-formula-control-btn:focus-visible {
		background: color-mix(in srgb, var(--gc-color-accent, #5d8cff) 20%, var(--gc-color-background));
		border-color: color-mix(in srgb, var(--gc-color-accent, #5d8cff) 58%, transparent);
		outline: none;
	}

	.processor-formula-control-btn:disabled {
		opacity: 0.45;
		cursor: default;
	}

	.processor-region-items {
		display: grid;
		gap: 0.18rem;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.processor-region-items li {
		padding: 0.18rem 0;
		font-size: 0.66rem;
	}

	.processor-region-empty {
		margin: 0;
		color: color-mix(in srgb, var(--gc-color-text) 48%, transparent);
		font-size: 0.64rem;
	}

	.graph-drop-zone {
		position: absolute;
		inset: 0;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
	}

	.builtin-formula-view {
		display: grid;
		place-items: center;
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		transition: padding-left 0.22s cubic-bezier(0.2, 0, 0.13, 1);
	}

	.builtin-formula-panel {
		display: grid;
		gap: 0.45rem;
		max-inline-size: 28rem;
		padding: 1rem;
		text-align: center;
	}

	.builtin-formula-panel strong {
		font-size: 1rem;
	}

	.builtin-formula-panel span {
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.68rem;
	}

	.builtin-formula-panel .processor-formula-control-btn {
		justify-self: center;
	}

	.formula-status,
	.save-indicator {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		font-size: 0.72rem;
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		min-inline-size: 1rem;
	}

	.formula-status {
		block-size: 1rem;
		border: 0.06rem solid currentColor;
		border-radius: 999rem;
		font-weight: 800;
		line-height: 1;
	}

	.formula-status.valid {
		color: color-mix(in srgb, var(--gc-color-success, #61d394) 88%, transparent);
	}

	.formula-status.error {
		color: var(--gc-color-error);
	}

	.save-indicator.error {
		color: var(--gc-color-error);
	}

	.empty-state {
		display: grid;
		place-content: center;
		block-size: 100%;
		padding: 2rem;
		text-align: center;
	}

	.empty-state p {
		max-inline-size: 30rem;
		margin: 0.4rem 0 0;
		color: color-mix(in srgb, var(--gc-color-text) 64%, transparent);
		font-size: 0.72rem;
	}

	.preview-status-bar {
		position: absolute;
		inset-inline-end: 0.75rem;
		inset-block-end: 0.75rem;
		z-index: 5;
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 0.4rem 0.55rem;
		max-inline-size: calc(100% - 1.5rem);
		padding: 0.28rem 0.5rem;
		border: 0.06rem solid
			color-mix(in srgb, var(--gc-color-background) 10%, rgba(255, 255, 255, 0.1));
		border-radius: 0.5rem;
		background: color-mix(in srgb, var(--gc-color-background) 84%, transparent);
		backdrop-filter: blur(0.5rem);
	}

	.diagnostics {
		position: absolute;
		inset-inline: 0.75rem;
		inset-block-end: 3.35rem;
		display: grid;
		max-block-size: 9rem;
		gap: 0.2rem;
		padding: 0.4rem;
		overflow-y: auto;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.35rem;
		background: color-mix(in srgb, var(--gc-color-background) 94%, transparent);
		box-shadow: 0 0.3rem 1rem color-mix(in srgb, black 25%, transparent);
	}

	.diagnostic {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		gap: 0.45rem;
		padding: 0.25rem 0.35rem;
		font-size: 0.64rem;
	}

	.diagnostic.error strong {
		color: var(--gc-color-error);
	}
</style>
