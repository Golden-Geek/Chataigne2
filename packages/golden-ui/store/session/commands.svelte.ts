import type { NodeId, ParamValue, UiEditIntent, UiNodeDto } from '../../types';
import type { GraphStore } from '../graph.svelte';
import { registerCommandHandler, type CommandId } from '../commands.svelte';
import { requestRemoveNodesById } from '../node-removal';
import {
	createNewProjectFile,
	openProjectFile,
	reopenLastProjectFile,
	saveProjectFile,
	saveProjectFileAs
} from '../project-files.svelte';
import { copyTextToClipboard } from '../../utils/clipboard';
import type { SelectionMode } from './types';
import {
	buildNodeTreeClipboardPayload,
	nodeTreeClipboardFromJson,
	nodeTreeClipboardJson,
	type NodeTreeClipboardNode
} from './node-tree-clipboard';

interface NodeTreeReferenceLookup {
	bySourceId: Map<NodeId, UiNodeDto>;
	bySourceUuid: Map<string, UiNodeDto>;
}

interface WorkbenchCommandSuiteOptions {
	graph: GraphStore;
	sendIntent(intent: UiEditIntent): Promise<void>;
	getSelectedNodes(): UiNodeDto[];
	getFirstSelectedNode(): UiNodeDto | null;
	getSelectedNodeIds(): NodeId[];
	selectNodes(nodeIds: NodeId[], selectionMode?: SelectionMode): void;
	revealNodeInInspector(nodeId: NodeId): boolean;
	undo(): Promise<void>;
	redo(): Promise<void>;
}

export interface WorkbenchCommandSuite {
	registerCommandHandlers(): () => void;
	reset(): void;
}

export const createWorkbenchCommandSuite = (
	options: WorkbenchCommandSuiteOptions
): WorkbenchCommandSuite => {
	let nodeClipboard = $state<NodeTreeClipboardNode[]>([]);
	const PREFERENCES_DECL_ID = 'preferences';

	const getNodeDepth = (nodeId: NodeId): number => {
		let depth = 0;
		let current = options.graph.state.parentById.get(nodeId);
		while (current !== undefined) {
			depth += 1;
			current = options.graph.state.parentById.get(current);
		}
		return depth;
	};

	const isNodeRemovable = (node: UiNodeDto): boolean => {
		if (options.graph.state.rootId === node.node_id) {
			return false;
		}
		return node.meta.user_permissions.can_remove_and_duplicate;
	};

	const isFolderNode = (node: UiNodeDto): boolean => node.node_type === 'folder';

	const isContainerNode = (node: UiNodeDto): boolean =>
		isFolderNode(node) || node.accepted_user_item_kinds.length > 0;

	const canParentCreateNodeType = (parentId: NodeId, nodeType: string): boolean => {
		const parent = options.graph.state.nodesById.get(parentId);
		if (!parent) {
			return false;
		}
		return parent.creatable_user_items.some((item) => item.node_type === nodeType);
	};

	const canParentAcceptItemKind = (parentId: NodeId, itemKind: string): boolean => {
		const parent = options.graph.state.nodesById.get(parentId);
		if (!parent) {
			return false;
		}
		return (
			parent.accepted_user_item_kinds.includes('*') ||
			parent.accepted_user_item_kinds.includes(itemKind)
		);
	};

	const waitForCreatedChild = async (
		parentId: NodeId,
		knownChildren: Set<NodeId>,
		expectedNodeType: string
	): Promise<NodeId | null> => {
		const deadline = Date.now() + 450;
		while (Date.now() <= deadline) {
			const parent = options.graph.state.nodesById.get(parentId);
			if (parent) {
				for (const childId of parent.children) {
					if (knownChildren.has(childId)) {
						continue;
					}
					const child = options.graph.state.nodesById.get(childId);
					if (!child) {
						continue;
					}
					if (child.node_type === expectedNodeType) {
						return childId;
					}
				}
			}
			await new Promise((resolve) => {
				setTimeout(resolve, 16);
			});
		}
		return null;
	};

	const createNodeUnderParent = async (
		parentId: NodeId,
		nodeType: string,
		label: string,
		insertAfterNodeId: NodeId | null
	): Promise<NodeId | null> => {
		const parentBefore = options.graph.state.nodesById.get(parentId);
		if (!parentBefore) {
			return null;
		}
		const knownChildren = new Set(parentBefore.children);
		await options.sendIntent({
			kind: 'createUserItem',
			parent: parentId,
			node_type: nodeType,
			label
		});
		const createdNodeId = await waitForCreatedChild(parentId, knownChildren, nodeType);
		if (createdNodeId === null || insertAfterNodeId === null) {
			return createdNodeId;
		}
		await options.sendIntent({
			kind: 'moveNode',
			node: createdNodeId,
			new_parent: parentId,
			new_prev_sibling: insertAfterNodeId
		});
		return createdNodeId;
	};

	const hasSelectedAncestor = (node: UiNodeDto, selectedIds: ReadonlySet<NodeId>): boolean => {
		let current = options.graph.state.parentById.get(node.node_id);
		while (current !== undefined) {
			if (selectedIds.has(current)) {
				return true;
			}
			current = options.graph.state.parentById.get(current);
		}
		return false;
	};

	const createReferenceLookup = (): NodeTreeReferenceLookup => ({
		bySourceId: new Map(),
		bySourceUuid: new Map()
	});

	const addReferenceTarget = (
		lookup: NodeTreeReferenceLookup,
		sourceTree: NodeTreeClipboardNode,
		targetNode: UiNodeDto
	): void => {
		lookup.bySourceId.set(sourceTree.sourceId, targetNode);
		lookup.bySourceUuid.set(sourceTree.sourceUuid, targetNode);
	};

	const matchingTargetChild = (
		targetParent: UiNodeDto,
		sourceTree: NodeTreeClipboardNode,
		claimedTargetIds: ReadonlySet<NodeId>
	): UiNodeDto | null => {
		const children = targetParent.children
			.map((childId) => options.graph.state.nodesById.get(childId) ?? null)
			.filter(
				(child): child is UiNodeDto => child !== null && !claimedTargetIds.has(child.node_id)
			);
		return (
			children.find(
				(child) => child.decl_id === sourceTree.decl_id && child.node_type === sourceTree.node_type
			) ??
			children.find((child) => child.decl_id === sourceTree.decl_id) ??
			null
		);
	};

	const materializeNodeTree = async (
		sourceTree: NodeTreeClipboardNode,
		targetNode: UiNodeDto,
		lookup: NodeTreeReferenceLookup
	): Promise<void> => {
		addReferenceTarget(lookup, sourceTree, targetNode);
		const claimedTargetIds = new Set<NodeId>();
		for (const sourceChild of sourceTree.children) {
			let targetChild = matchingTargetChild(targetNode, sourceChild, claimedTargetIds);
			if (!targetChild && canParentCreateNodeType(targetNode.node_id, sourceChild.node_type)) {
				const createdNodeId = await createNodeUnderParent(
					targetNode.node_id,
					sourceChild.node_type,
					sourceChild.label || sourceChild.node_type,
					null
				);
				targetChild =
					createdNodeId === null
						? null
						: (options.graph.state.nodesById.get(createdNodeId) ?? null);
			}
			if (!targetChild) {
				continue;
			}
			claimedTargetIds.add(targetChild.node_id);
			await materializeNodeTree(sourceChild, targetChild, lookup);
		}
	};

	const importedParamValue = (
		value: ParamValue,
		lookup: NodeTreeReferenceLookup
	): ParamValue | null => {
		if (value.kind !== 'reference') {
			return value;
		}
		const remapped =
			(value.cached_id === undefined ? undefined : lookup.bySourceId.get(value.cached_id)) ??
			lookup.bySourceUuid.get(value.uuid);
		if (remapped) {
			return {
				...value,
				uuid: remapped.uuid,
				cached_id: remapped.node_id,
				cached_name: remapped.meta.label
			};
		}
		for (const candidate of options.graph.state.nodesById.values()) {
			if (candidate.uuid !== value.uuid) continue;
			return {
				...value,
				cached_id: candidate.node_id,
				cached_name: candidate.meta.label
			};
		}
		return null;
	};

	const appendNodeTreeRestoreIntents = (
		sourceTree: NodeTreeClipboardNode,
		targetNode: UiNodeDto,
		lookup: NodeTreeReferenceLookup,
		intents: UiEditIntent[]
	): void => {
		const metaPatch: Partial<UiNodeDto['meta']> = {};
		if (
			targetNode.meta.user_permissions.can_edit_name &&
			sourceTree.meta.label.trim().length > 0 &&
			targetNode.meta.label !== sourceTree.meta.label
		) {
			metaPatch.label = sourceTree.meta.label;
		}
		if (targetNode.meta.can_be_disabled && targetNode.meta.enabled !== sourceTree.meta.enabled) {
			metaPatch.enabled = sourceTree.meta.enabled;
		}
		if (
			sourceTree.meta.presentation !== undefined &&
			JSON.stringify(targetNode.meta.presentation) !== JSON.stringify(sourceTree.meta.presentation)
		) {
			metaPatch.presentation = sourceTree.meta.presentation;
		}
		if (Object.keys(metaPatch).length > 0) {
			intents.push({ kind: 'patchMeta', node: targetNode.node_id, patch: metaPatch });
		}

		if (
			sourceTree.data.kind === 'parameter' &&
			targetNode.data.kind === 'parameter' &&
			!targetNode.data.param.read_only
		) {
			const value = importedParamValue(sourceTree.data.param.value, lookup);
			if (value && JSON.stringify(value) !== JSON.stringify(targetNode.data.param.value)) {
				intents.push({
					kind: 'setParam',
					node: targetNode.node_id,
					value,
					behaviour: targetNode.data.param.event_behaviour
				});
			}
			if (
				targetNode.meta.user_permissions.can_edit_constraints &&
				JSON.stringify(sourceTree.data.param.constraints) !==
					JSON.stringify(targetNode.data.param.constraints)
			) {
				intents.push({
					kind: 'setParamConstraints',
					node: targetNode.node_id,
					constraints: sourceTree.data.param.constraints
				});
			}
			if (
				JSON.stringify(sourceTree.data.param.control) !==
				JSON.stringify(targetNode.data.param.control)
			) {
				intents.push({
					kind: 'setParamControlState',
					node: targetNode.node_id,
					state: sourceTree.data.param.control
				});
			}
		}

		for (const sourceChild of sourceTree.children) {
			const targetChild =
				lookup.bySourceId.get(sourceChild.sourceId) ??
				lookup.bySourceUuid.get(sourceChild.sourceUuid) ??
				null;
			if (targetChild) {
				appendNodeTreeRestoreIntents(sourceChild, targetChild, lookup, intents);
			}
		}
	};

	const restoreNodeTree = async (
		sourceTree: NodeTreeClipboardNode,
		targetNode: UiNodeDto
	): Promise<void> => {
		const lookup = createReferenceLookup();
		await materializeNodeTree(sourceTree, targetNode, lookup);
		const intents: UiEditIntent[] = [];
		appendNodeTreeRestoreIntents(sourceTree, targetNode, lookup, intents);
		if (intents.length === 0) {
			return;
		}
		const clientEditId = `node-tree-paste-${Date.now().toString(36)}-${Math.random()
			.toString(36)
			.slice(2, 8)}`;
		await options.sendIntent({
			kind: 'beginEdit',
			client_edit_id: clientEditId,
			label: `Restore ${sourceTree.label || sourceTree.node_type}`
		});
		try {
			for (const intent of intents) {
				await options.sendIntent(intent);
			}
		} finally {
			await options.sendIntent({ kind: 'endEdit', client_edit_id: clientEditId }).catch(() => {});
		}
	};

	const readNodeClipboardFromSystem = async (): Promise<NodeTreeClipboardNode[] | null> => {
		if (typeof navigator === 'undefined' || !navigator.clipboard?.readText) {
			return null;
		}
		try {
			const payload = nodeTreeClipboardFromJson(await navigator.clipboard.readText());
			return payload?.nodes ?? null;
		} catch {
			return null;
		}
	};

	const removeSelectedNodesCommand = async (): Promise<boolean> => {
		const selected = options.getSelectedNodes();
		if (selected.length === 0) {
			return false;
		}

		const removable = selected.filter((node) => isNodeRemovable(node));
		if (removable.length === 0) {
			return false;
		}

		removable.sort((left, right) => getNodeDepth(right.node_id) - getNodeDepth(left.node_id));
		return requestRemoveNodesById(
			removable.map((node) => node.node_id),
			{
				graph: options.graph.state,
				sendIntent: options.sendIntent
			}
		);
	};

	const selectSiblingNodesCommand = (): boolean => {
		const anchorNode = options.getFirstSelectedNode();
		if (!anchorNode) {
			return false;
		}
		const parentId = options.graph.state.parentById.get(anchorNode.node_id);
		if (parentId === undefined) {
			return false;
		}
		const parent = options.graph.state.nodesById.get(parentId);
		if (!parent || parent.children.length === 0) {
			return false;
		}
		options.selectNodes([...parent.children], 'REPLACE');
		return true;
	};

	const copySelectedNodesCommand = (): boolean => {
		const selected = options.getSelectedNodes().filter(isNodeRemovable);
		if (selected.length === 0) {
			return false;
		}
		const selectedIds = new Set(selected.map((node) => node.node_id));
		const roots = selected
			.filter((node) => !hasSelectedAncestor(node, selectedIds))
			.sort((left, right) => getNodeDepth(left.node_id) - getNodeDepth(right.node_id));
		const payload = buildNodeTreeClipboardPayload(roots, options.graph.state.nodesById);
		nodeClipboard = payload.nodes;
		void copyTextToClipboard(nodeTreeClipboardJson(payload));
		return true;
	};

	const cutSelectedNodesCommand = async (): Promise<boolean> => {
		if (!copySelectedNodesCommand()) {
			return false;
		}
		return removeSelectedNodesCommand();
	};

	const duplicateSelectedNodesCommand = async (): Promise<boolean> => {
		const selectedSet = new Set(options.getSelectedNodeIds());
		const anchorNode = options.getFirstSelectedNode();
		if (!anchorNode) {
			return false;
		}
		const parentId = options.graph.state.parentById.get(anchorNode.node_id);
		if (parentId === undefined) {
			return false;
		}
		const parent = options.graph.state.nodesById.get(parentId);
		if (!parent) {
			return false;
		}

		const selectedSiblingIds = parent.children.filter((childId) => selectedSet.has(childId));
		if (selectedSiblingIds.length === 0) {
			return false;
		}

		let insertAfterNodeId: NodeId | null =
			selectedSiblingIds[selectedSiblingIds.length - 1] ?? null;
		const createdNodeIds: NodeId[] = [];
		for (const sourceId of selectedSiblingIds) {
			const source = options.graph.state.nodesById.get(sourceId);
			if (!source) {
				continue;
			}
			if (!source.meta.user_permissions.can_remove_and_duplicate) {
				continue;
			}
			if (!canParentAcceptItemKind(parentId, source.user_item_kind)) {
				continue;
			}

			const parentBefore = options.graph.state.nodesById.get(parentId);
			if (!parentBefore) {
				continue;
			}
			const knownChildren = new Set(parentBefore.children);
			await options.sendIntent({
				kind: 'duplicateNode',
				source: sourceId,
				new_parent: parentId,
				new_prev_sibling: insertAfterNodeId
			});
			const createdNodeId = await waitForCreatedChild(parentId, knownChildren, source.node_type);
			if (createdNodeId === null) {
				continue;
			}
			createdNodeIds.push(createdNodeId);
			insertAfterNodeId = createdNodeId;
		}

		if (createdNodeIds.length === 0) {
			return false;
		}
		options.selectNodes(createdNodeIds, 'REPLACE');
		return true;
	};

	const pasteNodesCommand = async (): Promise<boolean> => {
		const systemClipboard = await readNodeClipboardFromSystem();
		const clipboardNodes =
			systemClipboard && systemClipboard.length > 0 ? systemClipboard : nodeClipboard;
		if (clipboardNodes.length === 0) {
			return false;
		}
		nodeClipboard = clipboardNodes;

		const anchorNode = options.getFirstSelectedNode();
		if (!anchorNode) {
			return false;
		}

		let targetParentId: NodeId | undefined;
		let insertAfterNodeId: NodeId | null = null;
		if (isContainerNode(anchorNode)) {
			targetParentId = anchorNode.node_id;
		} else {
			targetParentId = options.graph.state.parentById.get(anchorNode.node_id);
			insertAfterNodeId = anchorNode.node_id;
		}
		if (targetParentId === undefined) {
			return false;
		}

		const createdNodeIds: NodeId[] = [];
		for (const entry of clipboardNodes) {
			const source = options.graph.state.nodesById.get(entry.sourceId);
			const sourceMatchesClipboard = source !== undefined && source.uuid === entry.sourceUuid;
			let createdNodeId: NodeId | null = null;
			let shouldRestoreTree = false;
			if (
				source &&
				sourceMatchesClipboard &&
				canParentAcceptItemKind(targetParentId, entry.user_item_kind)
			) {
				const parentBefore = options.graph.state.nodesById.get(targetParentId);
				if (!parentBefore) {
					continue;
				}
				const knownChildren = new Set(parentBefore.children);
				await options.sendIntent({
					kind: 'duplicateNode',
					source: source.node_id,
					new_parent: targetParentId,
					new_prev_sibling: insertAfterNodeId ?? undefined
				});
				createdNodeId = await waitForCreatedChild(targetParentId, knownChildren, source.node_type);
			} else if (canParentCreateNodeType(targetParentId, entry.node_type)) {
				createdNodeId = await createNodeUnderParent(
					targetParentId,
					entry.node_type,
					entry.label || entry.node_type,
					insertAfterNodeId
				);
				shouldRestoreTree = true;
			}
			if (createdNodeId === null) {
				continue;
			}
			if (shouldRestoreTree) {
				const createdNode = options.graph.state.nodesById.get(createdNodeId);
				if (createdNode) {
					await restoreNodeTree(entry, createdNode);
				}
			}
			createdNodeIds.push(createdNodeId);
			insertAfterNodeId = createdNodeId;
		}

		if (createdNodeIds.length === 0) {
			return false;
		}
		options.selectNodes(createdNodeIds, 'REPLACE');
		return true;
	};

	const preferencesNode = (): UiNodeDto | null => {
		const rootId = options.graph.state.rootId;
		if (rootId !== null) {
			const root = options.graph.state.nodesById.get(rootId);
			for (const childId of root?.children ?? []) {
				const child = options.graph.state.nodesById.get(childId);
				if (child?.decl_id === PREFERENCES_DECL_ID) {
					return child;
				}
			}
		}

		for (const node of options.graph.state.nodesById.values()) {
			if (node.decl_id === PREFERENCES_DECL_ID) {
				return node;
			}
		}
		return null;
	};

	const revealPreferencesCommand = (): boolean => {
		const preferences = preferencesNode();
		if (!preferences) {
			return false;
		}
		return options.revealNodeInInspector(preferences.node_id);
	};

	const registerCommandHandlers = (): (() => void) => {
		type CommandHandlerResult = boolean | void | Promise<boolean | void>;
		const cleanups: Array<() => void> = [];
		const bind = (
			commandId: CommandId,
			handler: () => CommandHandlerResult,
			priority = 10
		): void => {
			cleanups.push(registerCommandHandler(commandId, () => handler(), { priority }));
		};

		bind(
			'edit.undo',
			() => {
				void options.undo();
				return true;
			},
			20
		);
		bind(
			'edit.redo',
			() => {
				void options.redo();
				return true;
			},
			20
		);
		bind('edit.deleteSelection', () => removeSelectedNodesCommand(), 10);
		bind('select.all', () => selectSiblingNodesCommand(), 10);
		bind('edit.copy', () => copySelectedNodesCommand(), 10);
		bind('edit.cut', () => cutSelectedNodesCommand(), 10);
		bind('edit.duplicate', () => duplicateSelectedNodesCommand(), 10);
		bind('edit.paste', () => pasteNodesCommand(), 10);
		bind('file.new', () => createNewProjectFile(), 15);
		bind('file.save', () => saveProjectFile(), 15);
		bind('file.saveAs', () => saveProjectFileAs(), 15);
		bind('file.open', () => openProjectFile(), 15);
		bind('file.reopenLast', () => reopenLastProjectFile(), 15);
		bind('file.preferences', () => revealPreferencesCommand(), 15);

		return (): void => {
			for (const cleanup of cleanups) {
				cleanup();
			}
		};
	};

	const reset = (): void => {
		nodeClipboard = [];
	};

	return {
		registerCommandHandlers,
		reset
	};
};
