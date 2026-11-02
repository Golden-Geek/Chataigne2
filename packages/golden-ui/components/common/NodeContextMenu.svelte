<script lang="ts">
	import { onMount } from 'svelte';
	import { slide } from 'svelte/transition';
	import ContextMenu from './ContextMenu.svelte';
	import ColorPicker from './ColorPicker.svelte';
	import NodeConstraintMenu from './NodeConstraintMenu.svelte';
	import { appState } from '../../store/workbench.svelte';
	import { sendCreateUserItemIntent, sendPatchMetaIntent } from '../../store/ui-intents';
	import { requestRemoveNodesById } from '../../store/node-removal';
	import { executeCommand } from '../../store/commands.svelte';
	import type { ContextMenuAnchor, ContextMenuItem } from './context-menu';
	import {
		closeNodeContextMenu,
		nodeContextMenuState,
		openNodeContextMenu
	} from '../../store/node-context-menu.svelte';
	import { resolveNodeContextMenuItems } from './node-context-menu-registry';
	import type { NodeId, UiColorDto, UiCreatableUserItem, UiNodeDto } from '../../types';
	import { getMainViewportBounds, remToPx } from './floating-panel';
	import { buildCreatableItemMenu } from './creatable-item-menu';
	import { getPanelByType, showPanel } from '../../store/ui-panels';
	import { PERSISTED_PANEL_STATE_KEY } from '../../dockview/panel-persistence';
	import { copyTextToClipboard } from '../../utils/clipboard';
	import {
		buildNodeTreeClipboardPayload,
		nodeTreeClipboardJson
	} from '../../store/session/node-tree-clipboard';
	import addIcon from '../../style/icons/node/add.svg';
	import copyPathIcon from '../../style/icons/copy.svg';
	import colorIcon from '../../style/icons/parameter/color.svg';
	import settingsIcon from '../../style/icons/settings.svg';
	import duplicateIcon from '../../style/icons/duplicate.svg';
	import deleteIcon from '../../style/icons/delete.svg';
	import watchIcon from '../../style/icons/watch.svg';

	interface ResolvedNodeTarget {
		nodeId: NodeId;
		host: HTMLElement;
	}

	interface InspectorPanelPersistedState {
		inspectorLocked: boolean;
		inspectorLockedNodeId: NodeId;
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

	let session = $derived(appState.session);
	let graphState = $derived(session?.graph.state ?? null);
	let contextNodeId = $derived(nodeContextMenuState.nodeId);
	let contextPosition = $derived(nodeContextMenuState.position);
	let menuTreeDiv: HTMLDivElement | null = $state(null);
	let colorSubMenuDiv: HTMLDivElement | null = $state(null);
	let colorAnchorElement: HTMLElement | null = $state(null);

	let constraints = $state<ReturnType<typeof NodeConstraintMenu>>();
	let colorSubMenu = $state({ open: false });
	let isColorEditing = $state(false);

	const closeColorSubMenu = (): void => {
		colorSubMenu.open = false;
		colorAnchorElement = null;
		isColorEditing = false;
	};

	const closeMenu = (): void => {
		closeColorSubMenu();
		constraints?.close();
		closeNodeContextMenu();
	};

	const resolveNodeTarget = (target: EventTarget | null): ResolvedNodeTarget | null => {
		if (!(target instanceof Element)) {
			return null;
		}
		const host = target.closest<HTMLElement>('[data-node-id]');
		if (!host) {
			return null;
		}
		const rawNodeId = host.dataset.nodeId;
		if (!rawNodeId) {
			return null;
		}
		const parsed = Number(rawNodeId);
		if (!Number.isFinite(parsed)) {
			return null;
		}
		return {
			nodeId: parsed,
			host
		};
	};

	const applyPanelPosition = (node: HTMLElement, anchorElement: HTMLElement | null): void => {
		if (!anchorElement) {
			return;
		}

		const bounds = getMainViewportBounds();
		const paddingPx = remToPx(0.45);
		const gapPx = remToPx(0.35);
		const viewportMaxInlineSize = Math.max(1, bounds.width - paddingPx * 2);
		const maxBlockSize = Math.max(1, bounds.height - paddingPx * 2);
		node.style.setProperty('--gc-node-context-panel-max-inline-size', `${viewportMaxInlineSize}px`);
		node.style.maxInlineSize = `${viewportMaxInlineSize}px`;
		node.style.maxBlockSize = `${maxBlockSize}px`;

		const anchorRect = anchorElement.getBoundingClientRect();
		const menuRect =
			anchorElement.closest<HTMLElement>('.gc-context-menu')?.getBoundingClientRect() ?? anchorRect;
		const initialRect = node.getBoundingClientRect();

		const minLeft = bounds.left + paddingPx;
		const maxRight = bounds.right - paddingPx;
		const minTop = bounds.top + paddingPx;

		const rightSpace = Math.max(1, maxRight - menuRect.right - gapPx);
		const leftSpace = Math.max(1, menuRect.left - minLeft - gapPx);
		const minimumComfortWidth = remToPx(16);
		const preferredWidth = Math.max(initialRect.width, minimumComfortWidth);
		const shouldOpenLeft = rightSpace < preferredWidth;
		const sideSpace = shouldOpenLeft ? leftSpace : rightSpace;
		const availablePanelWidth =
			sideSpace >= minimumComfortWidth
				? Math.min(preferredWidth, sideSpace)
				: Math.min(preferredWidth, viewportMaxInlineSize);

		node.style.setProperty('--gc-node-context-panel-max-inline-size', `${availablePanelWidth}px`);
		node.style.maxInlineSize = `${availablePanelWidth}px`;

		const rect = node.getBoundingClientRect();
		const maxTop = Math.max(minTop, bounds.bottom - rect.height - paddingPx);

		if (shouldOpenLeft) {
			const viewportRight = typeof window === 'undefined' ? bounds.right : window.innerWidth;
			node.style.left = 'auto';
			node.style.right = `${Math.max(0, viewportRight - (menuRect.left - gapPx))}px`;
		} else {
			node.style.right = 'auto';
			node.style.left = `${Math.min(maxRight - rect.width, menuRect.right + gapPx)}px`;
		}
		node.style.top = `${Math.min(Math.max(minTop, anchorRect.top), maxTop)}px`;
	};

	const updateColorSubMenuPosition = (): void => {
		if (!colorSubMenuDiv || !colorSubMenu.open || !colorAnchorElement) {
			return;
		}
		applyPanelPosition(colorSubMenuDiv, colorAnchorElement);
	};

	const captureColorSubMenuContainer = () => {
		return (node: HTMLDivElement) => {
			colorSubMenuDiv = node;
			let raf = requestAnimationFrame(() => {
				updateColorSubMenuPosition();
			});
			return () => {
				cancelAnimationFrame(raf);
				if (colorSubMenuDiv === node) {
					colorSubMenuDiv = null;
				}
			};
		};
	};

	const captureMenuTree = (node: HTMLDivElement | null): void => {
		menuTreeDiv = node;
	};

	const isInsideCurrentMenuTree = (target: EventTarget | null): boolean => {
		if (!(target instanceof Node)) {
			return false;
		}
		return menuTreeDiv?.contains(target) === true || colorSubMenuDiv?.contains(target) === true;
	};

	const handleWindowContextMenu = (event: MouseEvent): void => {
		if (event.target instanceof Element && event.target.closest('[data-local-context-menu]')) {
			return;
		}
		event.preventDefault();
		event.stopPropagation();

		if (isInsideCurrentMenuTree(event.target)) {
			return;
		}

		const target = resolveNodeTarget(event.target);
		if (!target || !graphState?.nodesById.has(target.nodeId)) {
			closeMenu();
			return;
		}

		event.preventDefault();
		event.stopPropagation();

		let nextX = event.clientX;
		let nextY = event.clientY;
		if (nextX === 0 && nextY === 0) {
			const rect = target.host.getBoundingClientRect();
			nextX = rect.left + rect.width / 2;
			nextY = rect.top + rect.height / 2;
		}

		openNodeContextMenu(target.nodeId, nextX, nextY, {
			showChildCreation:
				target.host.closest<HTMLElement>('[data-node-context-child-creation]')?.dataset
					.nodeContextChildCreation !== 'false'
		});
		closeColorSubMenu();
	};

	const handleWindowViewportChange = (): void => {
		updateColorSubMenuPosition();
		constraints?.reposition();
	};

	onMount(() => {
		window.addEventListener('contextmenu', handleWindowContextMenu, true);
		window.addEventListener('resize', handleWindowViewportChange);
		window.addEventListener('scroll', handleWindowViewportChange, true);

		return () => {
			window.removeEventListener('contextmenu', handleWindowContextMenu, true);
			window.removeEventListener('resize', handleWindowViewportChange);
			window.removeEventListener('scroll', handleWindowViewportChange, true);
		};
	});

	let activeNode = $derived.by((): UiNodeDto | null => {
		if (contextNodeId === null || !graphState) {
			return null;
		}
		return graphState.nodesById.get(contextNodeId) ?? null;
	});

	$effect(() => {
		if (contextNodeId === null) {
			return;
		}
		if (activeNode !== null) {
			return;
		}
		closeMenu();
	});

	let parentNode = $derived.by((): UiNodeDto | null => {
		if (!activeNode || !graphState) {
			return null;
		}
		const parentId = graphState.parentById.get(activeNode.node_id);
		if (parentId === undefined) {
			return null;
		}
		return graphState.nodesById.get(parentId) ?? null;
	});

	let isRoot = $derived(Boolean(activeNode && graphState?.rootId === activeNode.node_id));
	let canDelete = $derived(
		Boolean(activeNode && activeNode.meta.user_permissions.can_remove_and_duplicate && !isRoot)
	);
	let canCopy = $derived(
		Boolean(activeNode && activeNode.meta.user_permissions.can_remove_and_duplicate && !isRoot)
	);
	let canDuplicate = $derived.by((): boolean => {
		if (!activeNode || !parentNode || isRoot) {
			return false;
		}
		if (!activeNode.meta.user_permissions.can_remove_and_duplicate) {
			return false;
		}
		return (
			parentNode.accepted_user_item_kinds.includes('*') ||
			parentNode.accepted_user_item_kinds.includes(activeNode.user_item_kind)
		);
	});
	let canSetColor = $derived(Boolean(activeNode?.meta.user_permissions.can_edit_color));
	let hasCustomColor = $derived(Boolean(activeNode?.meta.presentation?.color));
	let canSetConstraints = $derived(
		Boolean(
			activeNode?.meta.user_permissions.can_edit_constraints && activeNode.data.kind === 'parameter'
		)
	);
	let customNodeContextMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!activeNode) {
			return [];
		}
		return resolveNodeContextMenuItems({
			node: activeNode,
			parentNode,
			graphState,
			session,
			closeMenu,
			patchMeta: sendPatchMetaIntent
		});
	});
	let creatableItems = $derived(activeNode?.creatable_user_items ?? []);

	const clamp01 = (value: number): number => {
		if (!Number.isFinite(value)) {
			return 0;
		}
		return Math.max(0, Math.min(1, value));
	};

	const normalizeColor = (candidate: unknown): UiColorDto => {
		if (Array.isArray(candidate)) {
			return {
				r: clamp01(Number(candidate[0] ?? 0)),
				g: clamp01(Number(candidate[1] ?? 0)),
				b: clamp01(Number(candidate[2] ?? 0)),
				a: clamp01(Number(candidate[3] ?? 1))
			};
		}
		if (candidate && typeof candidate === 'object') {
			const value = candidate as { r?: unknown; g?: unknown; b?: unknown; a?: unknown };
			return {
				r: clamp01(Number(value.r ?? 0)),
				g: clamp01(Number(value.g ?? 0)),
				b: clamp01(Number(value.b ?? 0)),
				a: clamp01(Number(value.a ?? 1))
			};
		}
		return { r: 1, g: 1, b: 1, a: 1 };
	};

	const currentNodeColor = (node: UiNodeDto | null): UiColorDto => {
		const color = node?.meta.presentation?.color ?? node?.meta.presentation?.default_color;
		return {
			r: clamp01(color?.r ?? 1),
			g: clamp01(color?.g ?? 1),
			b: clamp01(color?.b ?? 1),
			a: clamp01(color?.a ?? 1)
		};
	};

	let activeNodeColor = $derived.by((): UiColorDto => {
		return currentNodeColor(activeNode);
	});
	let colorDraft = $state<UiColorDto>({ r: 1, g: 1, b: 1, a: 1 });

	$effect(() => {
		if (isColorEditing) {
			return;
		}
		colorDraft = activeNodeColor;
	});

	const relativeDeclPath = (targetNodeId: NodeId): string => {
		if (!graphState || graphState.rootId === null) {
			return '';
		}
		if (targetNodeId === graphState.rootId) {
			return '';
		}

		const reversed: string[] = [];
		let current: NodeId | undefined = targetNodeId;
		while (current !== undefined && current !== graphState.rootId) {
			const currentNode = graphState.nodesById.get(current);
			if (!currentNode) {
				return '';
			}
			reversed.push(currentNode.decl_id);
			current = graphState.parentById.get(current);
		}

		if (current !== graphState.rootId) {
			return '';
		}

		reversed.reverse();
		return reversed.join('/');
	};

	let scriptControlPath = $derived(activeNode ? relativeDeclPath(activeNode.node_id) : '');

	const copyScriptControlPath = (): void => {
		const value = scriptControlPath.length > 0 ? scriptControlPath : '.';
		void copyTextToClipboard(value);
	};

	const activeNodeClipboardJson = (): string | null => {
		if (!activeNode || !graphState) {
			return null;
		}
		return nodeTreeClipboardJson(buildNodeTreeClipboardPayload([activeNode], graphState.nodesById));
	};

	const nodeJsonFileName = (node: UiNodeDto): string => {
		const label = node.meta.label.trim().length > 0 ? node.meta.label : node.node_type;
		const stem = label
			.replace(/[<>:"/\\|?*\u0000-\u001f]+/g, '-')
			.replace(/\s+/g, '-')
			.replace(/^-+|-+$/g, '')
			.slice(0, 80);
		return `${stem || 'node'}.json`;
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

	const duplicateNode = (): void => {
		if (!session || !activeNode || !canDuplicate || !parentNode) {
			return;
		}
		void session
			.sendIntent({
				kind: 'duplicateNode',
				source: activeNode.node_id,
				new_parent: parentNode.node_id
			})
			.catch(() => {});
	};

	const deleteNode = (): void => {
		if (!session || !activeNode || !canDelete) {
			return;
		}
		void requestRemoveNodesById([activeNode.node_id]).catch(() => {});
	};

	const setNodeColor = (nextColor: UiColorDto): void => {
		if (!activeNode) {
			return;
		}
		const existingPresentation = activeNode.meta.presentation ?? {};
		void sendPatchMetaIntent(activeNode.node_id, {
			presentation: {
				...existingPresentation,
				color: nextColor
			}
		});
	};

	const resetNodeColor = (): void => {
		if (!activeNode || !canSetColor || !hasCustomColor) {
			return;
		}
		const presentation = { ...(activeNode.meta.presentation ?? {}) };
		delete presentation.color;
		void sendPatchMetaIntent(activeNode.node_id, { presentation });
	};

	const openColorSubMenu = (event: MouseEvent): void => {
		if (!canSetColor) {
			return;
		}
		const currentTarget = event.currentTarget as HTMLElement | null;
		if (!currentTarget) {
			return;
		}
		colorAnchorElement = currentTarget;
		colorSubMenu.open = true;

		queueMicrotask(() => {
			updateColorSubMenuPosition();
		});
	};

	const applyPickedColor = (nextColor: unknown): void => {
		if (!activeNode || !canSetColor) {
			return;
		}
		const normalized = normalizeColor(nextColor);
		colorDraft = normalized;
		setNodeColor(normalized);
	};

	const startColorEdit = (): void => {
		isColorEditing = true;
	};

	const endColorEdit = (): void => {
		isColorEditing = false;
	};

	const selectCopyScriptControlPath = (_event: MouseEvent): void => {
		copyScriptControlPath();
		closeMenu();
	};

	const selectCopyNode = (_event: MouseEvent): void => {
		if (!activeNode) {
			return;
		}
		session?.selectNode(activeNode.node_id, 'REPLACE');
		void executeCommand('edit.copy', { source: 'menu' });
		closeMenu();
	};

	const selectExportNodeAsJson = (_event: MouseEvent): void => {
		const json = activeNodeClipboardJson();
		if (json && activeNode) {
			void saveTextFile(nodeJsonFileName(activeNode), json).catch((error) => {
				if (error instanceof DOMException && error.name === 'AbortError') return;
				console.error('failed to export node JSON', error);
			});
		}
		closeMenu();
	};

	const selectPasteNode = (_event: MouseEvent): void => {
		if (!activeNode) {
			return;
		}
		session?.selectNode(activeNode.node_id, 'REPLACE');
		void executeCommand('edit.paste', { source: 'menu' });
		closeMenu();
	};

	const createUserItem = (item: UiCreatableUserItem): void => {
		if (!activeNode) {
			return;
		}
		void sendCreateUserItemIntent(activeNode.node_id, item).then((result) => {
			if (result.selectWhenCreated && result.createdNodeId !== null) {
				session?.selectNode(result.createdNodeId, 'REPLACE');
			}
		});
		closeMenu();
	};

	const selectSetColor = (event: MouseEvent): void => {
		if (colorSubMenu.open) {
			closeColorSubMenu();
			return;
		}
		openColorSubMenu(event);
	};

	const selectResetColor = (_event: MouseEvent): void => {
		resetNodeColor();
		closeMenu();
	};

	const selectDuplicateNode = (_event: MouseEvent): void => {
		duplicateNode();
		closeMenu();
	};

	const selectDeleteNode = (_event: MouseEvent): void => {
		deleteNode();
		closeMenu();
	};

	const spawnLockedInspector = (targetNode: UiNodeDto): void => {
		const firstInspectorPanel = getPanelByType('inspector');
		const panelId = `inspector-watch-${targetNode.node_id}-${Date.now().toString(36)}-${Math.floor(
			Math.random() * 0x100000
		).toString(36)}`;

		showPanel({
			panelType: 'inspector',
			panelId,
			title: `Inspector: ${targetNode.meta.label}`,
			params: {
				[PERSISTED_PANEL_STATE_KEY]: {
					inspectorLocked: true,
					inspectorLockedNodeId: targetNode.node_id
				} satisfies InspectorPanelPersistedState
			},
			position: firstInspectorPanel
				? {
						referencePanelId: firstInspectorPanel.panelId,
						direction: 'within'
					}
				: undefined
		});
	};

	let rootMenuItems = $derived.by((): ContextMenuItem[] => {
		if (!activeNode) {
			return [];
		}
		const showChildCreation = nodeContextMenuState.showChildCreation;
		const canPaste =
			showChildCreation &&
			(activeNode.accepted_user_item_kinds.length > 0 || creatableItems.length > 0);

		const items: ContextMenuItem[] = [
			...(showChildCreation && creatableItems.length > 0
				? [
						{
							id: 'create-child',
							label: 'Add',
							icon: addIcon,
							submenu: buildCreatableItemMenu(creatableItems, createUserItem)
						} satisfies ContextMenuItem,
						{ separator: true } satisfies ContextMenuItem
					]
				: []),
			{
				id: 'copy-script-control-path',
				label: 'Copy Script Control Path',
				icon: copyPathIcon,
				action: selectCopyScriptControlPath
			}
		];

		if (canSetColor || canSetConstraints) {
			items.push({ separator: true });
		}
		if (canSetColor) {
			items.push({
				id: 'set-color',
				label: 'Set Color',
				icon: colorIcon,
				className: colorSubMenu.open ? 'gc-node-context-item-open' : '',
				action: selectSetColor
			});
			items.push({
				id: 'reset-color',
				label: 'Reset Color',
				icon: colorIcon,
				disabled: !hasCustomColor,
				action: selectResetColor
			});
		}
		if (canSetConstraints) {
			items.push({
				id: 'set-constraints',
				label: 'Set Constraints',
				icon: settingsIcon,
				submenu: constraints?.menuItems(activeNode) ?? []
			});
		}
		if (customNodeContextMenuItems.length > 0) {
			items.push({ separator: true });
			items.push(...customNodeContextMenuItems);
		}

		if (canPaste || canCopy || canDuplicate || canDelete) {
			items.push({ separator: true });
		}
		if (canPaste) {
			items.push({
				id: 'paste',
				label: 'Paste',
				commandId: 'edit.paste',
				icon: copyPathIcon,
				action: selectPasteNode
			});
		}
		if (canCopy) {
			items.push({
				id: 'copy',
				label: 'Copy',
				commandId: 'edit.copy',
				icon: copyPathIcon,
				action: selectCopyNode
			});
			items.push({
				id: 'export-json',
				label: 'Export as JSON',
				icon: copyPathIcon,
				action: selectExportNodeAsJson
			});
		}
		if (canDuplicate) {
			items.push({
				id: 'duplicate',
				label: 'Duplicate',
				commandId: 'edit.duplicate',
				icon: duplicateIcon,
				action: selectDuplicateNode
			});
		}
		if (canDelete) {
			items.push({
				id: 'delete',
				label: 'Delete',
				commandId: 'edit.deleteSelection',
				color: 'color-mix(in srgb, #ff8b8b 82%, white 18%)',
				hoverColor: 'color-mix(in srgb, #ff5c5c 23%, transparent)',
				icon: deleteIcon,
				action: selectDeleteNode
			});
		}
		items.push({ separator: true });
		items.push({
			id: 'watch',
			label: 'Watch in Inspector',
			icon: watchIcon,
			action: () => {
				if (!activeNode) {
					return;
				}
				session?.selectNode(activeNode.node_id);
				closeMenu();
			}
		});
		items.push({
			id: 'watch-locked-inspector',
			label: 'Watch in New Locked Inspector',
			icon: watchIcon,
			action: () => {
				if (!activeNode) {
					return;
				}
				spawnLockedInspector(activeNode);
				closeMenu();
			}
		});

		return items;
	});

	let menuAnchor = $derived.by((): ContextMenuAnchor | null => {
		if (contextNodeId === null) {
			return null;
		}
		return {
			kind: 'point',
			x: contextPosition.x,
			y: contextPosition.y
		};
	});
	let menuItems = $derived(rootMenuItems);
	let showMenu = $derived(contextNodeId !== null && activeNode !== null && menuItems.length > 0);
	let showColorSubMenu = $derived(
		showMenu && colorSubMenu.open && canSetColor && colorAnchorElement !== null
	);

	$effect(() => {
		if (contextNodeId === null) {
			return;
		}
		if (menuItems.length > 0) {
			return;
		}
		closeMenu();
	});

	$effect(() => {
		if (canSetColor || !colorSubMenu.open) {
			return;
		}
		closeColorSubMenu();
	});

	$effect(() => {
		if (showMenu || !colorSubMenu.open) {
			return;
		}
		closeColorSubMenu();
	});

	$effect(() => {
		if (!showColorSubMenu) {
			return;
		}
		queueMicrotask(() => {
			updateColorSubMenuPosition();
		});
	});
</script>

<ContextMenu
	open={showMenu}
	items={menuItems}
	anchor={menuAnchor}
	insideElements={[menuTreeDiv, colorSubMenuDiv, ...(constraints?.insideElements() ?? [])]}
	closeOnContextMenuOutside={false}
	closeOnSelect={false}
	minWidthRem={12}
	zIndex={1300}
	menuClassName="gc-node-context-menu"
	onOpenChange={(nextOpen) => {
		if (!nextOpen) {
			closeMenu();
		}
	}}
	onMenuTreeMount={captureMenuTree} />

{#if showColorSubMenu}
	<div
		class="gc-node-context-color-submenu"
		{@attach captureColorSubMenuContainer()}
		transition:slide={{ axis: 'x', duration: 150 }}>
		<ColorPicker
			forceExpanded={true}
			color={colorDraft}
			onchange={(nextColor: unknown) => {
				applyPickedColor(nextColor);
			}}
			onStartEdit={startColorEdit}
			onEndEdit={endColorEdit} />
	</div>
{/if}

<NodeConstraintMenu
	bind:this={constraints}
	visible={showMenu}
	resolveNode={(id) => graphState?.nodesById.get(id) ?? null}
	positionPanel={applyPanelPosition}
	onclose={closeMenu} />

<style>
	:global(.gc-node-context-menu) {
		min-inline-size: 12rem;
	}

	:global(.gc-node-context-menu .gc-context-item) {
		font-size: 0.78rem;
	}

	:global(.gc-node-context-menu .gc-context-item.gc-node-context-item-open) {
		background: color-mix(in srgb, var(--gc-color-selection) 24%, transparent);
	}

	.gc-node-context-color-submenu {
		position: fixed;
		left: -10000px;
		top: -10000px;
		min-inline-size: min(16rem, var(--gc-node-context-panel-max-inline-size, calc(100vw - 0.9rem)));
		max-inline-size: min(
			24rem,
			42vw,
			var(--gc-node-context-panel-max-inline-size, calc(100vw - 0.9rem))
		);
		overflow: auto;
		overscroll-behavior: contain;
		box-sizing: border-box;
		z-index: 1301;
	}
</style>
