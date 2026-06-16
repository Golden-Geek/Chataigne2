<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		GraphCamera,
		GraphConnectionRequest,
		GraphNodeCreationRequest,
		GraphNodeMove,
		GraphNodePosition,
		GraphNodeResize
	} from 'golden_alchemist_ui';
	import type {
		ContextMenuAnchor,
		ContextMenuItem,
		NodeId,
		PanelProps,
		PanelState,
		UiCreateUserItemInitialParam,
		UiCreatableUserItem,
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
	import {
		createUiEditSession,
		sendCreateUserItemByTypeIntent,
		sendUiIntentBatch
	} from 'golden_ui/store/ui-intents';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		ANODE_CREATE_PREFIX,
		ANODE_NODE_TYPE,
		CONNECTION_NODE_TYPE,
		FORMULA_NODE_TYPE,
		MANAGER_REF_TYPE_CONDITIONS,
		MANAGER_REF_TYPE_INPUTS,
		MANAGER_REF_TYPE_OUTPUTS,
		PROPERTIES_DECL_ID,
		PROPERTY_FOLDER_NODE_TYPE,
		PROPERTY_MANAGER_NODE_TYPE,
		PROPERTY_NODE_TYPE,
		anodeCategoryColor,
		anodeDefaultColor,
		directChild,
		formulaANodes,
		canConnectGraphConnection,
		managerAnodeType,
		parameterChild,
		toGraphEdges,
		toGraphNodes
	} from '../alchemistGraph';
	import type {
		FormulaPreviewModeDto,
		ProcessorLaneSummaryDto,
		RuntimeValueDto,
		StateMachineProtocolBundle
	} from '../generated';
	import {
		STATE_MACHINE_RUNTIME_PREVIEW_TOPIC,
		formulaOutputPreviewMap,
		type FormulaOutputPreviewChip
	} from '../preview/formulaOutputPreviewStore.svelte';
	import { formulaPreviewSessionStore } from '../preview/formulaPreviewSessionStore.svelte';
	import AlchemistGraphEditor from './AlchemistGraphEditor.svelte';
	import AutoWireToggle from './AutoWireToggle.svelte';
	import FormulaPreviewModeSelector from './FormulaPreviewModeSelector.svelte';
	import ProcessorLaneSelector from './ProcessorLaneSelector.svelte';

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

	interface AlchemistEditorPanelPersistedState {
		autoWire?: boolean;
	}

	const PROPERTY_DRAG_TYPE = 'application/x-chataigne-alchemist-property';
	const MANAGER_DRAG_TYPE = 'application/x-chataigne-alchemist-manager';

	const MIN_PANEL_WIDTH = 160;
	const MAX_PANEL_WIDTH = 520;
	const DEFAULT_PANEL_WIDTH = 240;
	const FORMULA_CAMERA_STORAGE_PREFIX = 'chataigne.alchemist.formula_camera:';
	const HIDDEN_ANODE_CREATE_TYPES = new Set([
		`${ANODE_CREATE_PREFIX}property`,
		`${ANODE_CREATE_PREFIX}${MANAGER_REF_TYPE_CONDITIONS}`,
		`${ANODE_CREATE_PREFIX}${MANAGER_REF_TYPE_INPUTS}`,
		`${ANODE_CREATE_PREFIX}${MANAGER_REF_TYPE_OUTPUTS}`
	]);
	const PROCESSOR_ITEM_KIND = 'state_processor';
	const PREVIEW_ACTIVITY_HOLD_MS = 50;

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
	let persistenceTail = Promise.resolve();
	let previewActivityTimeout: ReturnType<typeof setTimeout> | null = null;
	let previewActivityDeadlines = new Map<string, number>();

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
		runtimeValueSignature(preview.value);

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
	let requestedProcessorLaneSummaries = $derived.by((): ProcessorLaneSummaryDto[] => {
		const value = panelState.params.processorLanes;
		return Array.isArray(value) ? (value as ProcessorLaneSummaryDto[]) : [];
	});

	let formulaNodes = $derived.by((): UiNodeDto[] => {
		if (!graphState) return [];
		return [...graphState.nodesById.values()]
			.filter(isFormula)
			.sort((left, right) => left.meta.label.localeCompare(right.meta.label));
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
		if (processorFormula) return processorFormula;
		if (previewTarget?.kind === 'formula') return previewTargetNode(previewTarget);
		return requestedFormula ?? formulaNodes[0] ?? null;
	});
	let runtimePreviewSequence = $derived(
		session?.getCustomEventSequence(STATE_MACHINE_RUNTIME_PREVIEW_TOPIC) ?? 0
	);
	let runtimePreviewBundle = $derived.by((): StateMachineProtocolBundle | null => {
		runtimePreviewSequence;
		return (
			session?.getCustomEventPayload<StateMachineProtocolBundle>(
				STATE_MACHINE_RUNTIME_PREVIEW_TOPIC
			) ?? null
		);
	});
	let runtimeProcessorLaneSummaries = $derived.by((): ProcessorLaneSummaryDto[] => {
		if (!processorNode || !runtimePreviewBundle) return [];
		return runtimePreviewBundle.processor_lanes.filter(
			(lane) => lane.processor_id === processorNode.uuid
		);
	});
	let processorLaneSummaries = $derived(
		requestedProcessor?.node_id === processorNode?.node_id &&
			requestedProcessorLaneSummaries.length > 0
			? requestedProcessorLaneSummaries
			: runtimeProcessorLaneSummaries
	);
	let previewSessionModel = $derived(
		formulaPreviewSessionStore.model(formula, processorNode, processorLaneSummaries)
	);
	let incomingOutputPreviews = $derived.by(() =>
		formulaOutputPreviewMap(
			formula,
			graphState?.nodesById ?? new Map(),
			runtimePreviewBundle,
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
		if (incomingOutputPreviews.size === 0) return;

		const next = new Map(outputPreviews);
		const updatedRefs: string[] = [];
		for (const [ref, preview] of incomingOutputPreviews) {
			const current = next.get(ref);
			if (current && preview.logicalTick < current.logicalTick) continue;
			const previewChanged =
				!current || previewActivitySignature(current) !== previewActivitySignature(preview);
			next.set(ref, preview);
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
	let anodeItems = $derived(
		formula?.creatable_user_items
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
			}) ?? []
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
	let contextMenuAnchor = $derived.by(
		(): ContextMenuAnchor => ({
			kind: 'point',
			x: contextMenuX,
			y: contextMenuY
		})
	);

	$effect(() => {
		props.panelApi.setTitle(formula ? `Alchemist: ${formula.meta.label}` : 'Alchemist Editor');
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
		if (node.node_type === PROPERTY_MANAGER_NODE_TYPE) {
			event.dataTransfer.setData(MANAGER_DRAG_TYPE, String(node.node_id));
		} else if (node.node_type === PROPERTY_NODE_TYPE) {
			event.dataTransfer.setData(PROPERTY_DRAG_TYPE, String(node.node_id));
		}
	};

	const initialParam = (
		decl_id: string,
		value: UiCreateUserItemInitialParam['value']
	): UiCreateUserItemInitialParam => ({ decl_id, value });

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
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				parent.node_id,
				item.node_type,
				item.label,
				{ select_when_created: true }
			);
			if (!result.success) throw new Error(`failed to create ${item.label}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const createPropertyGetter = (property: UiNodeDto, position: GraphNodePosition): void => {
		if (!formula || !graphState || property.node_type !== PROPERTY_NODE_TYPE) return;
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
							kind: 'str',
							value: property.uuid
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

	const getManagerRole = (manager: UiNodeDto): string => {
		if (!graphState) return '';
		const roleParam = parameterChild(manager, graphState.nodesById, 'role');
		if (roleParam?.data.kind === 'parameter' && roleParam.data.param.value.kind === 'enum') {
			return roleParam.data.param.value.value;
		}
		return '';
	};

	const createManagerNode = (manager: UiNodeDto, position: GraphNodePosition): void => {
		if (!formula || !graphState || manager.node_type !== PROPERTY_MANAGER_NODE_TYPE) return;
		const role = getManagerRole(manager);
		const typeId = managerAnodeType(role);
		if (!typeId) return;
		const managerNodeType = `${ANODE_CREATE_PREFIX}${typeId}`;
		if (
			!formula.creatable_user_items.some(
				(item: UiCreatableUserItem) => item.node_type === managerNodeType
			)
		)
			return;
		const managerItem = {
			node_type: managerNodeType,
			label: manager.meta.label
		};
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				formula.node_id,
				managerItem.node_type,
				manager.meta.label,
				{
					select_when_created: true,
					created_node_type: ANODE_NODE_TYPE,
					initial_params: [
						initialParam('position', {
							kind: 'vec2',
							value: [position.x, position.y]
						})
					]
				}
			);
			if (!result.success)
				throw new Error(`failed to create manager node for ${manager.meta.label}`);
			if (result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
	};

	const allowPropertyDrop = (event: DragEvent): void => {
		if (
			!event.dataTransfer?.types.includes(PROPERTY_DRAG_TYPE) &&
			!event.dataTransfer?.types.includes(MANAGER_DRAG_TYPE)
		)
			return;
		event.preventDefault();
		event.dataTransfer.dropEffect = 'copy';
	};

	const dropProperty = (event: DragEvent): void => {
		if (!graphState) return;
		const position = graphEditor?.clientToWorld(event.clientX, event.clientY) ?? { x: 0, y: 0 };

		const propertyId = Number(event.dataTransfer?.getData(PROPERTY_DRAG_TYPE));
		if (Number.isSafeInteger(propertyId) && propertyId !== 0) {
			const property = graphState.nodesById.get(propertyId);
			if (property?.node_type === PROPERTY_NODE_TYPE) {
				event.preventDefault();
				createPropertyGetter(property, position);
				return;
			}
		}

		const managerId = Number(event.dataTransfer?.getData(MANAGER_DRAG_TYPE));
		if (Number.isSafeInteger(managerId) && managerId !== 0) {
			const manager = graphState.nodesById.get(managerId);
			if (manager?.node_type === PROPERTY_MANAGER_NODE_TYPE) {
				event.preventDefault();
				createManagerNode(manager, position);
			}
		}
	};

	let contextMenuItems = $derived.by((): ContextMenuItem[] => {
		return buildCreatableItemMenu(anodeItems, (item) =>
			createNode(item, contextMenuWorldPosition ?? graphEditor?.viewportCenter() ?? { x: 0, y: 0 })
		);
	});

	const editParameters = async (label: string, intents: UiEditIntent[]): Promise<void> => {
		if (intents.length === 0) return;
		const editSession = createUiEditSession(label, 'alchemist-formula');
		const deadline = Date.now() + 750;
		while (!editSession.active && Date.now() <= deadline) {
			await editSession.begin();
			if (!editSession.active) {
				await new Promise((resolve) => setTimeout(resolve, 16));
			}
		}
		if (!editSession.active) throw new Error('could not start Alchemist edit session');
		try {
			const result = await sendUiIntentBatch(intents);
			if (!result.success)
				throw new Error(`${intents.length - result.appliedCount} edits were rejected`);
		} finally {
			await editSession.end();
		}
	};

	const moveNodes = (moves: GraphNodeMove[]): Promise<void> =>
		runMutation(async () => {
			if (!graphState || moves.length === 0) return;
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
		if (!formula || !graphState || !session) return false;
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
			if (!graphState) return;
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
			if (!graphState) return;
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
			if (!graphState) return;
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
			if (!graphState) return;
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
		if (!formula || !graphState) return;
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
	};

	const openCreateRequest = (request: GraphNodeCreationRequest): void => {
		showCreateMenu(request.clientX, request.clientY, request.position);
	};

	export const setPanelState = (next: PanelState): void => {
		updatedPanelState = next;
	};

	onMount(() => {
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
		return () => {
			unregisterFrame();
			unregisterHome();
			unregisterSelectAll();
			unregisterDeleteSelection();
		};
	});
</script>

{#snippet toolbarEndContent()}
	{#if formula}
		<FormulaPreviewModeSelector model={previewSessionModel} />
		<ProcessorLaneSelector
			lanes={previewSessionModel.lanes}
			selectedLaneId={previewSessionModel.selectedLaneId}
			onSelect={(laneId) =>
				formulaPreviewSessionStore.selectLane(processorNode?.node_id ?? null, laneId)} />
	{/if}
	{#if formula && anodeItems.length > 0}
		<NodeAddButton node={formula} items={anodeItems} onCreateItem={(item) => createNode(item)} />
	{/if}
	{#if formula}
		<AutoWireToggle checked={autoWire} onchange={setAutoWire} />
	{/if}
	{#if formula}
		<span
			class="formula-status"
			class:valid={formulaValid}
			class:error={!formulaValid}
			title={formulaStatusTitle}
			aria-label={formulaStatusTitle}>
			{formulaValid ? '✓' : '!'}
		</span>
	{/if}
	{#if saveStatus === 'saving' || saveStatus === 'error'}
		<span class="save-indicator" class:error={saveStatus === 'error'} aria-live="polite">
			{saveStatus === 'saving' ? '…' : '!'}
		</span>
	{/if}
{/snippet}

<section bind:this={panelRoot} class="alchemist-editor-panel" aria-label={panelState.title}>
	<div class="editor-content">
		{#if formula && graphState}
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
						<span class="properties-toggle-label">Properties</span>
					</button>
				</div>
				<ManagerListPanel
					managerNode={properties}
					addTargetNode={activePropertyContainer}
					addItems={activePropertyContainer?.creatable_user_items}
					searchPlaceholder="Search properties..."
					missingMessage="Formula properties are not available."
					emptyMessage="Drag a property onto the graph to create a getter."
					rootDropMessage="Drop here to move into Properties."
					addButtonTitle="Add property item"
					isTreeNode={isPropertyTreeNode}
					canRenderNodeChildren={canRenderPropertyChildren}
					nodeDraggable={canMovePropertyNode}
					onNodeDragStartData={setPropertyGraphDragData}
					onSelectNode={(n: UiNodeDto) => session?.selectNode(n.node_id, 'REPLACE')}
					onCreateItem={(parent, item) => createPropertyItem(parent, item)} />
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

			<div
				class="graph-drop-zone"
				role="application"
				aria-label="Alchemist graph drop target"
				ondragover={allowPropertyDrop}
				ondrop={dropProperty}>
				<AlchemistGraphEditor
					bind:this={graphEditor}
					{formula}
					nodesById={graphState.nodesById}
					{selectedNodeIds}
					{selectedEdgeIds}
					{outputPreviews}
					{activeSocketRefs}
					catalogItems={anodeItems}
					onGraphSelectionChange={selectGraphItems}
					onNodesMove={moveNodes}
					onNodeResize={resizeNode}
					onNodeRename={renameNode}
					onNodeCollapsedChange={setNodeCollapsed}
					onNodeEnabledChange={setNodeEnabled}
					onConnect={connectNodes}
					onBackgroundContextMenu={openContextMenu}
					onCreateRequest={openCreateRequest}
					initialCamera={formulaCamera}
					onCameraChange={setFormulaCamera}
					viewportInset={{ left: propertiesVisible ? propertiesWidth : 0 }}
					{autoWire}
					toolbarEnd={toolbarEndContent} />
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
		z-index: 20;
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
		z-index: 25;
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

	.graph-drop-zone {
		position: absolute;
		inset: 0;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
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

	.diagnostics {
		position: absolute;
		inset-inline: 0.75rem;
		inset-block-end: 0.75rem;
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
