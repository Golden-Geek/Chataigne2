<script lang="ts">
	import type { GraphCamera, GraphNodeCreationRequest, GraphNodePosition } from 'golden_graph_ui';
	import { createFormulaCameras } from '../editor/formulaCameras.svelte';
	import FormulaPropertiesPanel from './FormulaPropertiesPanel.svelte';
	import { createFormulaMutations } from '../editor/formulaMutations';
	import { createFormulaGraphEdits } from '../editor/formulaGraphEdits';
	import { createFormulaItems, PROPERTY_DRAG_TYPE } from '../editor/formulaItems.svelte';
	import { createFormulaEditorClipboard } from '../editor/formulaEditorClipboard.svelte';
	import { createFormulaEditorPreview } from '../editor/formulaEditorPreview.svelte';

	import { onMount } from 'svelte';
	import type {
		ContextMenuAnchor,
		ContextMenuItem,
		NodeId,
		PanelProps,
		PanelState,
		UiNodeDto
	} from 'golden_ui';
	import { ContextMenu, buildCreatableItemMenu } from 'golden_ui';
	import {
		readPanelPersistedState,
		writePanelPersistedState
	} from 'golden_ui/dockview/panel-persistence';
	import { registerCommandHandler } from 'golden_ui/store/commands.svelte';
	import { appState } from 'golden_ui/store/workbench.svelte';
	import {
		ANODE_CREATE_PREFIX,
		ANODE_NODE_TYPE,
		FORMULA_NODE_TYPE,
		PROPERTIES_DECL_ID,
		PROPERTY_MANAGER_NODE_TYPE,
		PROPERTY_NODE_TYPE,
		anodeCategoryColor,
		anodeDefaultColor,
		directChild,
		formulaANodes,
		formulaConnections,
		parameterChild
	} from '../alchemistGraph';
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

	const HIDDEN_ANODE_CREATE_TYPES = new Set([`${ANODE_CREATE_PREFIX}property`]);

	const PROCESSOR_ITEM_KIND = 'state_processor';
	const FORMULA_LIBRARY_NODE_TYPE = 'alchemist_formula_library';

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
	let graphEditor = $state<ReturnType<typeof AlchemistGraphEditor> | null>(null);
	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let saveStatus = $state<'idle' | 'saving' | 'saved' | 'error'>('idle');
	let propertiesVisible = $state(true);
	let propertiesInset = $state(0);
	const cameras = createFormulaCameras();
	let autoWire = $state(true);
	let initializedAutoWirePanelId: string | null = null;
	let previewTarget = $state<PreviewTarget | null>(null);

	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuWorldPosition: GraphNodePosition | null = null;

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

	const isFormula = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.node_type === FORMULA_NODE_TYPE;

	const isProcessor = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.user_item_kind === PROCESSOR_ITEM_KIND || node?.node_type === PROCESSOR_ITEM_KIND;

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
	let formulaSource = $derived(graphState ? formulaSourceKind(formula, graphState) : 'project');
	let formulaSourceDisplay = $derived(getFormulaSourceDisplay(formulaSource));
	let formulaExternalFile = $derived(formulaIsExternalFile(formula));
	let formulaReadOnly = $derived(formulaIsReadOnly(formula));
	$effect(() => {
		void session?.snapshotGeneration;
		void formula?.uuid;
		saveStatus = 'idle';
	});
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
	let formulaCamera = $derived.by((): GraphCamera | undefined => {
		if (!formula) return undefined;
		return cameras.read(formula.uuid);
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
				? formulaConnections(formula, graphState.nodesById).map((node) => node.node_id)
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
		const hasSelection = selectedClipboardNodeIds().length > 0;
		const editItems: ContextMenuItem[] = [];
		if (hasSelection) {
			editItems.push(
				{
					id: 'copy-selection',
					label: 'Copy',
					commandId: 'edit.copy',
					action: () => {
						void copySelectedGraphItems();
						contextMenuOpen = false;
					}
				},
				{
					id: 'export-selection-json',
					label: 'Export as JSON',
					action: () => {
						void exportSelectedAlchemistClipboard();
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

	async function removeSelectedGraphItems(): Promise<boolean> {
		if (!session || !formula || formulaReadOnly) return false;
		const selected = session.selectedNodesIds.filter(
			(node) => anodeNodeIds.has(node) || connectionNodeIds.has(node)
		);
		if (selected.length === 0) return false;
		await graphEdits.remove(selected.map(String));
		return true;
	}

	const mutations = createFormulaMutations(
		() => ({ session, formula, readOnly: formulaReadOnly }),
		(value) => (saveStatus = value)
	);
	const graphEdits = createFormulaGraphEdits(mutations);
	const moveNodes = graphEdits.move,
		resizeNode = graphEdits.resize,
		renameNode = graphEdits.rename,
		setNodeCollapsed = graphEdits.collapse,
		setNodeEnabled = graphEdits.enable,
		connectNodes = graphEdits.connect;
	const preview = createFormulaEditorPreview(() => ({
		session,
		formula,
		processorNode,
		manager: stateMachineManager
	}));
	let processorUi = $derived(preview.processorUi),
		previewSessionModel = $derived(preview.model);
	let outputPreviews = $derived(preview.outputPreviews),
		activeSocketRefs = $derived(preview.activeSocketRefs);
	const publishPreviewDemand = preview.publishPreviewDemand;
	const items = createFormulaItems(mutations, () => ({
		formula,
		readOnly: formulaReadOnly,
		items: anodeItems,
		formulas: formulaNodes,
		library: formulaLibrary,
		processor: processorUi,
		graph: graphState,
		center: () => graphEditor?.viewportCenter(),
		closeMenu: () => (contextMenuOpen = false),
		setPreviewTarget
	}));
	const {
		createNode,
		createPropertyItem,
		createEditableBuiltInFormulaCopy,
		createPropertyGetter,
		createManagerGetter
	} = items;
	const clipboard = createFormulaEditorClipboard(
		mutations,
		() => ({
			session,
			formula,
			readOnly: formulaReadOnly,
			anodeNodeIds,
			center: () => graphEditor?.viewportCenter()
		}),
		removeSelectedGraphItems
	);
	const {
		selectedClipboardNodeIds,
		copySelectedGraphItems,
		duplicateSelectedGraphItems,
		pasteGraphItems,
		cutSelectedGraphItems,
		exportSelectedAlchemistClipboard
	} = clipboard;

	const setFormulaCamera = (camera: GraphCamera): void => {
		if (!formula) return;
		cameras.write(formula.uuid, camera);
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

	export const moveGraphSelection = (key: string, large = false) =>
		graphEditor?.moveGraphSelection(key, large) ?? false;
	export const connectGraphSocket = (node: string, socket: string, direction: 'input' | 'output') =>
		graphEditor?.connectGraphSocket(node, socket, direction) ?? false;
	export const revealGraphNode = (id: string, additive = false) =>
		graphEditor?.revealGraphNode(id, additive) ?? Promise.resolve(false);
	export const collapseGraphNode = (id: string) => graphEditor?.collapseGraphNode(id) ?? false;
	export const graphInteractionState = () => graphEditor?.graphInteractionState();
	export const findGraphNodes = (query: string) => graphEditor?.findGraphNodes(query);

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
			mutations.dispose();
			clipboard.dispose();
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
		{#if graphState && (formula || processorNode)}
			<FormulaPropertiesPanel
				{processorNode}
				{formula}
				{properties}
				{activePropertyContainer}
				{formulaReadOnly}
				{createPropertyItem}
				bind:visible={propertiesVisible}
				onInsetChange={(value) => (propertiesInset = value)} />

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
						viewportInset={{ left: propertiesInset }}
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
			{:else if processorNode}
				<div
					class="builtin-formula-view"
					style:padding-left={propertiesVisible ? `${propertiesInset}px` : '0'}>
					<div class="builtin-formula-panel">
						<strong>{processorUi?.formula_label ?? processorNode.meta.label}</strong>
						<span
							>{processorUi?.formula_source_kind === 'builtin'
								? 'Built-in, read-only'
								: 'Formula source unavailable'}</span>
						{#if processorUi?.formula_can_duplicate_to_library}
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
