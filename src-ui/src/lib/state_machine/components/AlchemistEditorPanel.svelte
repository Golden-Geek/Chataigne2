<script lang="ts">
	import { onMount } from 'svelte';
	import type {
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
		OutlinerDropTarget,
		OutlinerDropZone,
		PanelProps,
		PanelState,
		UiCreateUserItemInitialParam,
		UiCreatableUserItem,
		UiEditIntent,
		UiNodeDto
	} from 'golden_ui';
	import {
		ContextMenu,
		NodeAddButton,
		OutlinerItem,
		buildCreatableItemMenu,
		canDragOutlinerNode,
		resolveOutlinerDropTarget
	} from 'golden_ui';
	import {
		createUiEditSession,
		sendCreateUserItemByTypeIntent,
		sendMoveNodeIntent,
		sendUiIntentBatch
	} from 'golden_ui/store/ui-intents';
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
		directChild,
		formulaANodes,
		managerAnodeType,
		parameterChild,
		toGraphEdges
	} from '../alchemistGraph';
	import AlchemistGraphEditor from './AlchemistGraphEditor.svelte';

	const DIAGNOSTICS_DECL_ID = 'diagnostics_json';
	const VALID_DECL_ID = 'is_valid';

	interface FormulaDiagnostic {
		code: string;
		message: string;
		severity: 'info' | 'warning' | 'error';
		origin: string;
	}

	const PROPERTY_DRAG_TYPE = 'application/x-chataigne-alchemist-property';
	const MANAGER_DRAG_TYPE = 'application/x-chataigne-alchemist-manager';

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
	let contextMenuOpen = $state(false);
	let contextMenuX = $state(0);
	let contextMenuY = $state(0);
	let contextMenuWorldPosition: GraphNodePosition | null = null;
	let persistenceTail = Promise.resolve();
	let activePropertyDragNodeId = $state<NodeId | null>(null);
	let propertyDropTarget = $state<OutlinerDropTarget | null>(null);
	let propertyMoveInFlight = $state(false);

	const isFormula = (node: UiNodeDto | null | undefined): node is UiNodeDto =>
		node?.node_type === FORMULA_NODE_TYPE;

	let requestedFormulaNodeId = $derived.by(() => {
		const value = panelState.params.formulaNodeId;
		const parsed = typeof value === 'number' ? value : Number(value);
		return Number.isInteger(parsed) ? parsed : null;
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
			let currentId: number | undefined = selectedId;
			while (currentId !== undefined) {
				const current = graphState.nodesById.get(currentId);
				if (isFormula(current)) return current;
				currentId = graphState.parentById.get(currentId);
			}
		}
		return null;
	});

	let formula = $derived.by((): UiNodeDto | null => {
		if (!graphState) return null;
		const requested =
			requestedFormulaNodeId === null ? null : graphState.nodesById.get(requestedFormulaNodeId);
		return selectedFormula ?? (isFormula(requested) ? requested : (formulaNodes[0] ?? null));
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
	let anodeItems = $derived(
		formula?.creatable_user_items.filter((item) =>
			item.node_type.startsWith(ANODE_CREATE_PREFIX)
		) ?? []
	);
	let properties = $derived(
		graphState ? directChild(formula, graphState.nodesById, PROPERTIES_DECL_ID) : null
	);
	let isPropertyRootDropActive = $derived(
		properties !== null && propertyDropTarget?.hoverNodeId === properties.node_id
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

	const clearPropertyDragState = (): void => {
		activePropertyDragNodeId = null;
		propertyDropTarget = null;
	};

	const isPropertyRowTarget = (target: EventTarget | null): boolean =>
		target instanceof Element && target.closest('.outliner-item-content') !== null;

	const resolvePropertyDropZone = (event: DragEvent): OutlinerDropZone => {
		const row = event.currentTarget;
		if (!(row instanceof HTMLElement)) {
			return 'inside';
		}
		const bounds = row.getBoundingClientRect();
		const ratio = (event.clientY - bounds.top) / Math.max(bounds.height, 1);
		return ratio <= 0.3 ? 'before' : ratio >= 0.7 ? 'after' : 'inside';
	};

	const setPropertyGraphDragData = (node: UiNodeDto, event: DragEvent): void => {
		if (!event.dataTransfer) return;
		event.dataTransfer.effectAllowed = 'copyMove';
		if (node.node_type === PROPERTY_MANAGER_NODE_TYPE) {
			event.dataTransfer.setData(MANAGER_DRAG_TYPE, String(node.node_id));
		} else if (node.node_type === PROPERTY_NODE_TYPE) {
			event.dataTransfer.setData(PROPERTY_DRAG_TYPE, String(node.node_id));
		}
	};

	const handlePropertyNodeDragStart = (node: UiNodeDto, event: DragEvent): void => {
		setPropertyGraphDragData(node, event);
		if (!canMovePropertyNode(node) || propertyMoveInFlight) {
			clearPropertyDragState();
			return;
		}
		activePropertyDragNodeId = node.node_id;
		propertyDropTarget = null;
	};

	const handlePropertyNodeDragOver = (hoverNode: UiNodeDto, event: DragEvent): void => {
		if (propertyMoveInFlight || activePropertyDragNodeId === null) {
			propertyDropTarget = null;
			return;
		}
		const next = resolveOutlinerDropTarget(
			graphState,
			activePropertyDragNodeId,
			hoverNode.node_id,
			resolvePropertyDropZone(event)
		);
		if (!next) {
			propertyDropTarget = null;
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
		propertyDropTarget = next;
	};

	const commitPropertyDrop = async (
		sourceNodeId: NodeId,
		next: OutlinerDropTarget | null,
		event: DragEvent
	): Promise<void> => {
		clearPropertyDragState();
		if (!next) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();
		propertyMoveInFlight = true;
		try {
			await sendMoveNodeIntent(sourceNodeId, next.newParentId, next.newPrevSiblingId ?? undefined);
		} finally {
			propertyMoveInFlight = false;
			clearPropertyDragState();
		}
	};

	const handlePropertyNodeDrop = async (hoverNode: UiNodeDto, event: DragEvent): Promise<void> => {
		const sourceNodeId = activePropertyDragNodeId;
		if (sourceNodeId === null || propertyMoveInFlight) {
			clearPropertyDragState();
			return;
		}
		await commitPropertyDrop(
			sourceNodeId,
			resolveOutlinerDropTarget(
				graphState,
				sourceNodeId,
				hoverNode.node_id,
				resolvePropertyDropZone(event)
			),
			event
		);
	};

	const handlePropertyRootDragOver = (event: DragEvent): void => {
		if (
			propertyMoveInFlight ||
			activePropertyDragNodeId === null ||
			!properties ||
			isPropertyRowTarget(event.target)
		) {
			return;
		}
		const next = resolveOutlinerDropTarget(
			graphState,
			activePropertyDragNodeId,
			properties.node_id,
			'inside'
		);
		if (!next) {
			propertyDropTarget = null;
			return;
		}
		event.preventDefault();
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
		propertyDropTarget = next;
	};

	const handlePropertyRootDrop = async (event: DragEvent): Promise<void> => {
		const sourceNodeId = activePropertyDragNodeId;
		if (
			sourceNodeId === null ||
			propertyMoveInFlight ||
			!properties ||
			isPropertyRowTarget(event.target)
		) {
			return;
		}
		await commitPropertyDrop(
			sourceNodeId,
			resolveOutlinerDropTarget(graphState, sourceNodeId, properties.node_id, 'inside'),
			event
		);
	};

	const selectFormula = (event: Event): void => {
		const formulaNodeId = Number((event.currentTarget as HTMLSelectElement).value);
		if (!Number.isInteger(formulaNodeId)) return;
		const params = { ...panelState.params, formulaNodeId };
		updatedPanelState = { ...panelState, params };
		props.panelApi.updateParams(params);
		session?.selectNode(formulaNodeId, 'REPLACE');
		saveStatus = 'idle';
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
		if (!formula || !graphState || !anodeItems.includes(item)) return;
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

	const propertyValueType = (property: UiNodeDto): string => {
		if (!graphState) return 'float';
		const value = parameterChild(property, graphState.nodesById, 'value');
		if (value?.data.kind !== 'parameter') return 'float';
		switch (value.data.param.value.kind) {
			case 'trigger':
			case 'int':
			case 'float':
			case 'bool':
			case 'vec2':
			case 'vec3':
			case 'color':
				return value.data.param.value.kind;
			case 'reference':
				return 'chataigne.module_endpoint';
			default:
				return 'string';
		}
	};

	const createPropertyGetter = (property: UiNodeDto, position: GraphNodePosition): void => {
		if (!formula || !graphState || property.node_type !== PROPERTY_NODE_TYPE) return;
		const propertyItem = anodeItems.find(
			(item) => item.node_type === `${ANODE_CREATE_PREFIX}property`
		);
		const value = parameterChild(property, graphState.nodesById, 'value');
		if (!propertyItem || value?.data.kind !== 'parameter') return;
		const initialValue = value.data.param.value;
		void runMutation(async () => {
			const result = await sendCreateUserItemByTypeIntent(
				formula.node_id,
				propertyItem.node_type,
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
						}),
						initialParam('config/value__type', {
							kind: 'enum',
							value: propertyValueType(property)
						}),
						initialParam('config/value', initialValue)
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
		const managerItem = anodeItems.find(
			(item: UiCreatableUserItem) => item.node_type === `${ANODE_CREATE_PREFIX}${typeId}`
		);
		if (!managerItem) return;
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
		await editSession.begin();
		if (!editSession.active) throw new Error('another edit session is already active');
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

	const connectNodes = (connection: GraphConnectionRequest): void => {
		if (!formula || !graphState) return;
		const source = graphState.nodesById.get(Number(connection.from.nodeId));
		const target = graphState.nodesById.get(Number(connection.to.nodeId));
		if (source?.node_type !== ANODE_NODE_TYPE || target?.node_type !== ANODE_NODE_TYPE) return;
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
					edge.to.nodeId === connection.to.nodeId && edge.to.socketId === connection.to.socketId
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
		graphEditor?.focus();
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
		return () => {
			unregisterFrame();
			unregisterHome();
			unregisterSelectAll();
		};
	});
</script>

<section
	bind:this={panelRoot}
	class="alchemist-editor-panel"
	aria-label={panelState.title}
	onpointerdown={() => graphEditor?.focus()}>
	<header>
		<div class="identity">
			<strong>{formula?.meta.label ?? 'No Formula selected'}</strong>
			<span class:valid={formulaValid} class:invalid={!formulaValid && formula !== null}>
				{formula
					? formulaValid
						? 'Compiles successfully'
						: 'Has compile errors'
					: 'Formula authoring'}
			</span>
		</div>
		{#if formulaNodes.length > 0}
			<label class="picker">
				<span>Formula</span>
				<select value={formula?.node_id ?? ''} onchange={selectFormula}>
					{#each formulaNodes as option (option.node_id)}
						<option value={option.node_id}>{option.meta.label}</option>
					{/each}
				</select>
			</label>
		{/if}
		<button
			type="button"
			class="properties-toggle"
			class:active={propertiesVisible}
			aria-pressed={propertiesVisible}
			onclick={() => (propertiesVisible = !propertiesVisible)}>
			Properties
		</button>
		{#if formula && anodeItems.length > 0}
			<div class="node-add">
				<NodeAddButton
					node={formula}
					items={anodeItems}
					onCreateItem={(item) => createNode(item)} />
			</div>
		{/if}
		<span class:error={saveStatus === 'error'} class="save-status">
			{saveStatus === 'saving'
				? 'Saving...'
				: saveStatus === 'saved'
					? 'Saved'
					: saveStatus === 'error'
						? 'Edit failed'
						: ''}
		</span>
	</header>

	<div class:properties-visible={propertiesVisible} class="editor-content">
		{#if formula && graphState}
			{#if propertiesVisible}
				<aside class="properties-panel" aria-label="Formula properties">
					<div class="properties-heading">
						<div>
							<strong>Properties</strong>
							<span>Drag a property onto the graph to create a getter.</span>
						</div>
						{#if activePropertyContainer && activePropertyContainer.creatable_user_items.length > 0}
							<NodeAddButton
								node={activePropertyContainer}
								items={activePropertyContainer.creatable_user_items}
								onCreateItem={(item) => createPropertyItem(activePropertyContainer, item)} />
						{/if}
					</div>
					<div
						class="property-tree"
						class:root-drop-active={isPropertyRootDropActive}
						role="tree"
						tabindex="0"
						aria-label="Formula property hierarchy"
						ondragover={handlePropertyRootDragOver}
						ondrop={(event) => void handlePropertyRootDrop(event)}
						ondragleave={() => {
							if (isPropertyRootDropActive) {
								propertyDropTarget = null;
							}
						}}>
						{#if properties && properties.children.some((id) => graphState?.nodesById.get(id)?.data.kind !== 'parameter')}
							{#each properties.children as childId (childId)}
								{@const child = graphState?.nodesById.get(childId)}
								{#if child && child.data.kind !== 'parameter'}
									<OutlinerItem
										node={child}
										mode="tree"
										canRenderNodeChildren={canRenderPropertyChildren}
										nodeFilter={isPropertyTreeNode}
										nodeDraggable={canMovePropertyNode}
										activeDragNodeId={activePropertyDragNodeId}
										dropTarget={propertyDropTarget}
										onNodeDragStart={handlePropertyNodeDragStart}
										onNodeDragOver={handlePropertyNodeDragOver}
										onNodeDrop={handlePropertyNodeDrop}
										onNodeDragEnd={() => {
											if (!propertyMoveInFlight) {
												clearPropertyDragState();
											}
										}}
										onSelectNode={(n: UiNodeDto) => session?.selectNode(n.node_id, 'REPLACE')} />
								{/if}
							{/each}
						{:else}
							<p class="properties-empty">The Formula properties hierarchy is empty.</p>
						{/if}
					</div>
				</aside>
			{/if}
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
					catalogItems={anodeItems}
					onGraphSelectionChange={selectGraphItems}
					onNodesMove={moveNodes}
					onNodeResize={resizeNode}
					onNodeRename={renameNode}
					onNodeCollapsedChange={setNodeCollapsed}
					onConnect={connectNodes}
					onBackgroundContextMenu={openContextMenu}
					onCreateRequest={openCreateRequest} />
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
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		inline-size: 100%;
		block-size: 100%;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
		color: var(--gc-color-text);
		background: var(--gc-color-background);
	}

	header {
		display: flex;
		align-items: center;
		gap: 0.65rem;
		min-block-size: 2.7rem;
		padding: 0.4rem 0.65rem;
		border-block-end: 0.06rem solid var(--gc-color-border);
		background: var(--gc-color-background-soft);
	}

	.identity {
		display: grid;
		min-inline-size: 8rem;
		gap: 0.1rem;
	}

	.identity strong {
		font-size: 0.75rem;
	}

	.identity span,
	.picker span,
	.save-status {
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.62rem;
	}

	.identity .valid {
		color: var(--gc-color-success, #58b878);
	}

	.identity .invalid,
	.save-status.error {
		color: var(--gc-color-error);
	}

	.picker {
		display: flex;
		align-items: center;
		gap: 0.35rem;
	}

	.picker select {
		min-block-size: 1.65rem;
		padding: 0.2rem 0.45rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: var(--gc-color-text);
		font: inherit;
		font-size: 0.66rem;
	}

	.properties-toggle {
		min-block-size: 1.65rem;
		padding: 0.2rem 0.5rem;
		border: 0.06rem solid var(--gc-color-border);
		border-radius: 0.3rem;
		background: var(--gc-color-background);
		color: color-mix(in srgb, var(--gc-color-text) 68%, transparent);
		font: inherit;
		font-size: 0.66rem;
		cursor: pointer;
	}

	.properties-toggle.active {
		border-color: color-mix(in srgb, var(--gc-color-accent) 55%, var(--gc-color-border));
		color: var(--gc-color-text);
		background: color-mix(in srgb, var(--gc-color-accent) 14%, var(--gc-color-background));
	}

	.node-add {
		margin-inline-start: auto;
	}

	.save-status {
		min-inline-size: 3.5rem;
		text-align: end;
	}

	.editor-content {
		position: relative;
		display: grid;
		grid-template-columns: minmax(0, 1fr);
		min-inline-size: 0;
		min-block-size: 0;
	}

	.editor-content.properties-visible {
		grid-template-columns: minmax(13rem, 18rem) minmax(0, 1fr);
	}

	.properties-panel {
		display: grid;
		grid-template-rows: auto minmax(0, 1fr);
		min-inline-size: 0;
		min-block-size: 0;
		border-inline-end: 0.06rem solid var(--gc-color-border);
		background: var(--gc-color-background-soft);
	}

	.properties-heading {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 0.5rem;
		padding: 0.6rem 0.65rem;
		border-block-end: 0.06rem solid var(--gc-color-border);
	}

	.properties-heading div {
		display: grid;
		gap: 0.15rem;
	}

	.properties-heading strong {
		font-size: 0.75rem;
	}

	.properties-heading span,
	.properties-empty {
		color: color-mix(in srgb, var(--gc-color-text) 58%, transparent);
		font-size: 0.62rem;
		line-height: 1.35;
	}

	.property-tree {
		min-block-size: 0;
		overflow: auto;
		padding: 0.25rem;
		outline: 0.08rem solid transparent;
		outline-offset: -0.08rem;
	}

	.property-tree.root-drop-active {
		background: color-mix(in srgb, var(--gc-color-selection) 8%, transparent);
		outline-color: color-mix(in srgb, var(--gc-color-selection) 48%, transparent);
	}

	.properties-empty {
		margin: 0;
		padding: 0.75rem;
	}

	.graph-drop-zone {
		position: relative;
		min-inline-size: 0;
		min-block-size: 0;
		overflow: hidden;
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
