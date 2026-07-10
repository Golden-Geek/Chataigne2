<script lang="ts">
	import { onMount } from 'svelte';
	import { cubicOut } from 'svelte/easing';
	import type { Snippet } from 'svelte';
	import { scale } from 'svelte/transition';
	import {
		clampNumber,
		computePointAnchoredMenuPlacement,
		getMainViewportBounds,
		remToPx
	} from './floating-panel';
	import { getCommandShortcutHint } from '../../store/commands.svelte';
	import {
		normalizeContextMenuItems,
		type ContextMenuAnchor,
		type ContextMenuItem,
		type ContextMenuPlacement
	} from './context-menu';

	interface MenuLayer {
		depth: number;
		items: ContextMenuItem[];
		parentDepth: number | null;
		parentIndex: number | null;
	}

	interface ContextMenuProps {
		open?: boolean;
		items?: readonly ContextMenuItem[];
		anchor?: ContextMenuAnchor | null;
		usePortal?: boolean;
		preferPointerAnchor?: boolean;
		insideElements?: ReadonlyArray<HTMLElement | null>;
		closeOnOutsidePointerDown?: boolean;
		closeOnEscape?: boolean;
		closeOnContextMenuOutside?: boolean;
		closeOnViewportChange?: boolean;
		closeOnSelect?: boolean;
		minWidthRem?: number;
		maxWidthCss?: string;
		viewportPaddingRem?: number;
		submenuGapRem?: number;
		zIndex?: number;
		className?: string;
		menuClassName?: string;
		submenuClassName?: string;
		itemClassName?: string;
		itemContent?: Snippet<[ContextMenuItem, number, boolean]>;
		onOpenChange?: (nextOpen: boolean) => void;
		onClose?: () => void;
		onMenuTreeMount?: (node: HTMLDivElement | null) => void;
	}

	type MenuOriginX = 'left' | 'right';
	type MenuOriginY = 'top' | 'bottom';

	interface MenuPosition {
		left: number;
		top: number;
		originX: MenuOriginX;
		originY: MenuOriginY;
	}

	interface MenuViewportMetrics {
		availableWidth: number;
		availableHeight: number;
	}

	const estimatedMenuRowHeightRem = 1.95;
	const menuColumnGapRem = 0.25;
	const preferredMenuColumnWidthRem = 22;

	let {
		open = $bindable(false),
		items = [],
		anchor = null,
		usePortal = true,
		preferPointerAnchor = true,
		insideElements = [],
		closeOnOutsidePointerDown = true,
		closeOnEscape = true,
		closeOnContextMenuOutside = true,
		closeOnViewportChange = true,
		closeOnSelect = true,
		minWidthRem = 10,
		maxWidthCss = 'max-content',
		viewportPaddingRem = 0.45,
		submenuGapRem = 0.3,
		zIndex = 1300,
		className = '',
		menuClassName = '',
		submenuClassName = '',
		itemClassName = '',
		itemContent,
		onOpenChange = (_nextOpen: boolean): void => {},
		onClose = (): void => {},
		onMenuTreeMount = (_node: HTMLDivElement | null): void => {}
	}: ContextMenuProps = $props();

	let menuLayerNode: HTMLDivElement | null = $state(null);
	let submenuPath = $state<number[]>([]);
	let menuPositions = $state<Record<number, MenuPosition>>({});
	let renderedOpen = $state(false);
	let closing = $state(false);

	const menuNodes = new Map<number, HTMLDivElement>();
	const itemNodes = new Map<string, HTMLButtonElement>();
	let layoutRaf = 0;
	let closeTimer = 0;
	let lastAnchorPointer = $state<{ x: number; y: number; time: number } | null>(null);
	let latchedAnchorPointer = $state<{ x: number; y: number } | null>(null);

	const submenuItemsFor = (item: ContextMenuItem): ContextMenuItem[] => {
		return normalizeContextMenuItems(item.submenu);
	};

	const hasSubmenu = (item: ContextMenuItem): boolean => {
		return submenuItemsFor(item).length > 0;
	};

	const itemShortcutHint = (item: ContextMenuItem): string | undefined => {
		if (item.hint && item.hint.trim().length > 0) {
			return item.hint;
		}
		if (!item.commandId) {
			return undefined;
		}
		return getCommandShortcutHint(item.commandId);
	};

	let rootItems = $derived.by(() => normalizeContextMenuItems(items));

	let menuLayers = $derived.by((): MenuLayer[] => {
		const layers: MenuLayer[] = [];
		if (rootItems.length === 0) {
			return layers;
		}

		layers.push({
			depth: 0,
			items: rootItems,
			parentDepth: null,
			parentIndex: null
		});

		for (let depth = 0; depth < submenuPath.length; depth += 1) {
			const parentLayer = layers[depth];
			if (!parentLayer) {
				break;
			}
			const parentIndex = submenuPath[depth];
			const parentItem = parentLayer.items[parentIndex];
			if (!parentItem || parentItem.disabled) {
				break;
			}
			const nextItems = submenuItemsFor(parentItem);
			if (nextItems.length === 0) {
				break;
			}
			layers.push({
				depth: depth + 1,
				items: nextItems,
				parentDepth: depth,
				parentIndex
			});
		}

		return layers;
	});

	const setOpen = (nextOpen: boolean): void => {
		if (open === nextOpen) {
			return;
		}
		open = nextOpen;
		onOpenChange(nextOpen);
		if (!nextOpen) {
			submenuPath = [];
			menuPositions = {};
			onClose();
		}
	};

	const cancelCloseTimer = (): void => {
		if (closeTimer === 0) {
			return;
		}
		clearTimeout(closeTimer);
		closeTimer = 0;
	};

	const isTargetInsideMenuTree = (target: EventTarget | null): boolean => {
		if (!(target instanceof Node)) {
			return false;
		}
		if (menuLayerNode?.contains(target) === true) {
			return true;
		}
		for (const extraElement of insideElements) {
			if (extraElement?.contains(target) === true) {
				return true;
			}
		}
		return false;
	};

	const clearSubmenusFromDepth = (depth: number): void => {
		if (submenuPath.length <= depth) {
			return;
		}
		submenuPath = submenuPath.slice(0, depth);
	};

	const defaultMenuPosition = (left: number, top: number): MenuPosition => ({
		left,
		top,
		originX: 'left',
		originY: 'top'
	});

	const menuViewportMetrics = (): MenuViewportMetrics => {
		const bounds = getMainViewportBounds();
		const marginPx = remToPx(viewportPaddingRem);
		return {
			availableWidth: Math.max(1, bounds.width - marginPx * 2),
			availableHeight: Math.max(1, bounds.height - marginPx * 2)
		};
	};

	const rowCountForMenu = (itemCount: number, metrics: MenuViewportMetrics): number => {
		const visibleItemCount = Math.max(1, itemCount);
		const rowHeightPx = Math.max(1, remToPx(estimatedMenuRowHeightRem));
		const rowsByHeight = Math.max(1, Math.floor(metrics.availableHeight / rowHeightPx));
		const columnGapPx = remToPx(menuColumnGapRem);
		const minColumnWidthPx = Math.max(1, Math.min(metrics.availableWidth, remToPx(minWidthRem)));
		const columnsByWidth = Math.max(
			1,
			Math.floor((metrics.availableWidth + columnGapPx) / (minColumnWidthPx + columnGapPx))
		);
		const rowsNeededForAvailableWidth = Math.ceil(visibleItemCount / columnsByWidth);
		const preferredRows = Math.min(visibleItemCount, rowsByHeight);
		return clampNumber(Math.max(preferredRows, rowsNeededForAvailableWidth), 1, visibleItemCount);
	};

	const menuStyle = (layer: MenuLayer, position: MenuPosition): string => {
		const metrics = menuViewportMetrics();
		const rowCount = rowCountForMenu(layer.items.length, metrics);
		const wrapsIntoColumns = rowCount < layer.items.length;
		const defaultColumnWidthPx = Math.min(
			metrics.availableWidth,
			Math.max(remToPx(minWidthRem), remToPx(preferredMenuColumnWidthRem))
		);
		const columnWidthCss =
			maxWidthCss.trim() === 'max-content'
				? wrapsIntoColumns
					? `${defaultColumnWidthPx}px`
					: 'max-content'
				: maxWidthCss;
		return [
			`left:${position.left}px`,
			`top:${position.top}px`,
			`--gc-context-min-width:${minWidthRem}rem`,
			`--gc-context-max-width:${maxWidthCss}`,
			`--gc-context-column-width:${columnWidthCss}`,
			`--gc-context-available-width:${metrics.availableWidth}px`,
			`--gc-context-available-height:${metrics.availableHeight}px`,
			`--gc-context-row-count:${rowCount}`,
			`--gc-context-column-gap:${menuColumnGapRem}rem`,
			`--gc-context-origin-x:${position.originX}`,
			`--gc-context-origin-y:${position.originY}`
		].join(';');
	};

	const originForElementPlacement = (
		placement: ContextMenuPlacement
	): { originX: MenuOriginX; originY: MenuOriginY } => {
		if (placement.startsWith('left')) {
			return {
				originX: 'right',
				originY: placement.endsWith('end') ? 'bottom' : 'top'
			};
		}
		if (placement.startsWith('right')) {
			return {
				originX: 'left',
				originY: placement.endsWith('end') ? 'bottom' : 'top'
			};
		}
		return {
			originX: placement.endsWith('end') ? 'right' : 'left',
			originY: placement.startsWith('top') ? 'bottom' : 'top'
		};
	};

	const resolveElementPlacement = (
		rect: DOMRect,
		menuWidth: number,
		menuHeight: number,
		placement: ContextMenuPlacement,
		gapPx: number
	): { left: number; top: number } => {
		switch (placement) {
			case 'bottom-end':
				return {
					left: rect.right - menuWidth,
					top: rect.bottom + gapPx
				};
			case 'top-start':
				return {
					left: rect.left,
					top: rect.top - menuHeight - gapPx
				};
			case 'top-end':
				return {
					left: rect.right - menuWidth,
					top: rect.top - menuHeight - gapPx
				};
			case 'right-start':
				return {
					left: rect.right + gapPx,
					top: rect.top
				};
			case 'right-end':
				return {
					left: rect.right + gapPx,
					top: rect.bottom - menuHeight
				};
			case 'left-start':
				return {
					left: rect.left - menuWidth - gapPx,
					top: rect.top
				};
			case 'left-end':
				return {
					left: rect.left - menuWidth - gapPx,
					top: rect.bottom - menuHeight
				};
			case 'bottom-start':
			default:
				return {
					left: rect.left,
					top: rect.bottom + gapPx
				};
		}
	};

	const computeRootMenuPosition = (menuNode: HTMLDivElement): MenuPosition | null => {
		if (!anchor) {
			return null;
		}

		const bounds = getMainViewportBounds();
		const width = menuNode.getBoundingClientRect().width;
		const height = menuNode.getBoundingClientRect().height;

		if (anchor.kind === 'point') {
			const placement = computePointAnchoredMenuPlacement({
				bounds,
				anchorX: anchor.x,
				anchorY: anchor.y,
				menuWidth: width,
				menuHeight: height,
				marginRem: viewportPaddingRem
			});
			return {
				left: placement.left,
				top: placement.top,
				originX: placement.originX,
				originY: placement.originY
			};
		}

		if (!anchor.element) {
			return null;
		}

		if (preferPointerAnchor) {
			if (latchedAnchorPointer) {
				const placement = computePointAnchoredMenuPlacement({
					bounds,
					anchorX: latchedAnchorPointer.x,
					anchorY: latchedAnchorPointer.y,
					menuWidth: width,
					menuHeight: height,
					marginRem: viewportPaddingRem
				});
				return {
					left: placement.left,
					top: placement.top,
					originX: placement.originX,
					originY: placement.originY
				};
			} else if (lastAnchorPointer) {
				const pointerAgeMs = performance.now() - lastAnchorPointer.time;
				if (pointerAgeMs <= 500) {
					latchedAnchorPointer = { x: lastAnchorPointer.x, y: lastAnchorPointer.y };
					const placement = computePointAnchoredMenuPlacement({
						bounds,
						anchorX: latchedAnchorPointer.x,
						anchorY: latchedAnchorPointer.y,
						menuWidth: width,
						menuHeight: height,
						marginRem: viewportPaddingRem
					});
					return {
						left: placement.left,
						top: placement.top,
						originX: placement.originX,
						originY: placement.originY
					};
				}
			}
		}

		const rect = anchor.element.getBoundingClientRect();
		const gapPx = remToPx(anchor.offsetRem ?? 0.25);
		const requestedPlacement = anchor.placement ?? 'bottom-start';
		const desired = resolveElementPlacement(rect, width, height, requestedPlacement, gapPx);
		const origin = originForElementPlacement(requestedPlacement);

		const marginPx = remToPx(viewportPaddingRem);
		const minLeft = bounds.left + marginPx;
		const maxLeft = Math.max(minLeft, bounds.right - width - marginPx);
		const minTop = bounds.top + marginPx;
		const maxTop = Math.max(minTop, bounds.bottom - height - marginPx);

		return {
			left: clampNumber(desired.left, minLeft, maxLeft),
			top: clampNumber(desired.top, minTop, maxTop),
			originX: origin.originX,
			originY: origin.originY
		};
	};

	const computeSubMenuPosition = (
		parentNode: HTMLButtonElement,
		menuNode: HTMLDivElement
	): MenuPosition => {
		const bounds = getMainViewportBounds();
		const marginPx = remToPx(viewportPaddingRem);
		const gapPx = remToPx(submenuGapRem);

		const parentRect = parentNode.getBoundingClientRect();
		const menuRect = menuNode.getBoundingClientRect();

		const minLeft = bounds.left + marginPx;
		const maxLeft = Math.max(minLeft, bounds.right - menuRect.width - marginPx);
		const minTop = bounds.top + marginPx;
		const maxTop = Math.max(minTop, bounds.bottom - menuRect.height - marginPx);

		const rightSpace = bounds.right - marginPx - parentRect.right - gapPx;
		const leftSpace = parentRect.left - bounds.left - marginPx - gapPx;
		const shouldOpenLeft = rightSpace < menuRect.width && leftSpace > rightSpace;
		let left = shouldOpenLeft ? parentRect.left - menuRect.width - gapPx : parentRect.right + gapPx;

		if (!shouldOpenLeft && left > maxLeft && leftSpace > rightSpace) {
			left = parentRect.left - menuRect.width - gapPx;
		}

		const clampedLeft = clampNumber(left, minLeft, maxLeft);
		const top = clampNumber(parentRect.top, minTop, maxTop);
		return {
			left: clampedLeft,
			top,
			originX: clampedLeft + menuRect.width <= parentRect.left ? 'right' : 'left',
			originY: top < parentRect.top ? 'bottom' : 'top'
		};
	};

	const updateLayout = (): void => {
		if (!open) {
			return;
		}

		const rootNode = menuNodes.get(0);
		if (!rootNode) {
			return;
		}

		const rootPosition = computeRootMenuPosition(rootNode);
		if (!rootPosition) {
			return;
		}

		const nextPositions: Record<number, MenuPosition> = {
			0: rootPosition
		};

		for (const layer of menuLayers) {
			if (layer.depth === 0) {
				continue;
			}
			const parentDepth = layer.parentDepth;
			const parentIndex = layer.parentIndex;
			if (parentDepth === null || parentIndex === null) {
				continue;
			}

			const parentNode = itemNodes.get(`${parentDepth}:${parentIndex}`);
			const menuNode = menuNodes.get(layer.depth);
			if (!parentNode || !menuNode) {
				continue;
			}

			nextPositions[layer.depth] = computeSubMenuPosition(parentNode, menuNode);
		}

		menuPositions = nextPositions;
	};

	const scheduleLayout = (): void => {
		if (!open) {
			return;
		}
		if (layoutRaf !== 0) {
			cancelAnimationFrame(layoutRaf);
		}
		layoutRaf = requestAnimationFrame(() => {
			layoutRaf = 0;
			updateLayout();
		});
	};

	const handleGlobalPointerDown = (event: PointerEvent): void => {
		if (!open || !closeOnOutsidePointerDown) {
			return;
		}
		if (isTargetInsideMenuTree(event.target)) {
			return;
		}
		setOpen(false);
	};

	const handleGlobalKeyDown = (event: KeyboardEvent): void => {
		if (!open || !closeOnEscape) {
			return;
		}
		if (event.key === 'Escape') {
			event.preventDefault();
			setOpen(false);
		}
	};

	const handleGlobalContextMenu = (event: MouseEvent): void => {
		if (!open || !closeOnContextMenuOutside) {
			return;
		}
		if (isTargetInsideMenuTree(event.target)) {
			return;
		}
		setOpen(false);
	};

	const handleViewportChange = (): void => {
		if (!open || !closeOnViewportChange) {
			return;
		}
		scheduleLayout();
	};

	const handleMenuContextMenu = (event: MouseEvent): void => {
		event.preventDefault();
		event.stopPropagation();
	};

	const captureMenuLayerNode = (node: HTMLDivElement) => {
		menuLayerNode = node;
		onMenuTreeMount(node);
		scheduleLayout();

		return () => {
			if (menuLayerNode === node) {
				menuLayerNode = null;
				onMenuTreeMount(null);
			}
		};
	};

	const portalMenuLayer = (node: HTMLDivElement) => {
		if (!usePortal || typeof document === 'undefined') {
			return;
		}

		document.body.appendChild(node);

		return () => {
			node.remove();
		};
	};

	const handleAnchorPointerDown = (event: PointerEvent): void => {
		lastAnchorPointer = {
			x: event.clientX,
			y: event.clientY,
			time: performance.now()
		};
	};

	const captureMenuNode = (depth: number) => {
		return (node: HTMLDivElement) => {
			menuNodes.set(depth, node);
			scheduleLayout();
			return () => {
				menuNodes.delete(depth);
			};
		};
	};

	const captureItemNode = (depth: number, index: number) => {
		return (node: HTMLButtonElement) => {
			itemNodes.set(`${depth}:${index}`, node);
			return () => {
				itemNodes.delete(`${depth}:${index}`);
			};
		};
	};

	const handleItemPointerEnter = (depth: number, index: number, item: ContextMenuItem): void => {
		if (item.disabled) {
			clearSubmenusFromDepth(depth);
			return;
		}

		if (!hasSubmenu(item)) {
			clearSubmenusFromDepth(depth);
			return;
		}

		const nextPath = submenuPath.slice(0, depth);
		nextPath[depth] = index;
		submenuPath = nextPath;
		scheduleLayout();
	};

	const handleItemClick = (
		event: MouseEvent,
		depth: number,
		index: number,
		item: ContextMenuItem
	): void => {
		event.stopPropagation();
		if (item.disabled || item.separator) {
			return;
		}

		if (hasSubmenu(item)) {
			if (submenuPath[depth] === index) {
				submenuPath = submenuPath.slice(0, depth);
			} else {
				const nextPath = submenuPath.slice(0, depth);
				nextPath[depth] = index;
				submenuPath = nextPath;
			}
			scheduleLayout();
			return;
		}

		item.action?.(event, item);
		const shouldClose = item.closeOnSelect ?? closeOnSelect;
		if (shouldClose) {
			setOpen(false);
		}
	};

	$effect(() => {
		if (open) {
			return;
		}
		submenuPath = [];
		menuPositions = {};
		latchedAnchorPointer = null;
	});

	$effect(() => {
		if (!open) {
			return;
		}
		if (!anchor || rootItems.length === 0) {
			setOpen(false);
		}
	});

	$effect(() => {
		const canRender = open && anchor !== null && rootItems.length > 0;
		if (canRender) {
			cancelCloseTimer();
			closing = false;
			renderedOpen = true;
			return;
		}

		if (!renderedOpen) {
			return;
		}

		if (closing) {
			return;
		}

		closing = true;
		cancelCloseTimer();
		closeTimer = window.setTimeout(() => {
			closeTimer = 0;
			closing = false;
			renderedOpen = false;
			menuPositions = {};
			submenuPath = [];
		}, 180);
	});

	$effect(() => {
		if (!preferPointerAnchor || !anchor || anchor.kind !== 'element' || !anchor.element) {
			return;
		}

		const anchorElement = anchor.element;
		anchorElement.addEventListener('pointerdown', handleAnchorPointerDown, true);
		return () => {
			anchorElement.removeEventListener('pointerdown', handleAnchorPointerDown, true);
		};
	});

	$effect(() => {
		if (!open) {
			return;
		}
		void anchor;
		void menuLayers;
		scheduleLayout();
	});

	$effect(() => {
		if (!open || typeof document === 'undefined' || typeof window === 'undefined') {
			return;
		}

		document.addEventListener('pointerdown', handleGlobalPointerDown, true);
		document.addEventListener('keydown', handleGlobalKeyDown, true);
		if (closeOnContextMenuOutside) {
			window.addEventListener('contextmenu', handleGlobalContextMenu, true);
		}
		if (closeOnViewportChange) {
			window.addEventListener('resize', handleViewportChange);
			window.addEventListener('scroll', handleViewportChange, true);
		}

		return () => {
			document.removeEventListener('pointerdown', handleGlobalPointerDown, true);
			document.removeEventListener('keydown', handleGlobalKeyDown, true);
			if (closeOnContextMenuOutside) {
				window.removeEventListener('contextmenu', handleGlobalContextMenu, true);
			}
			if (closeOnViewportChange) {
				window.removeEventListener('resize', handleViewportChange);
				window.removeEventListener('scroll', handleViewportChange, true);
			}
		};
	});

	onMount(() => {
		return () => {
			cancelCloseTimer();
			if (layoutRaf !== 0) {
				cancelAnimationFrame(layoutRaf);
				layoutRaf = 0;
			}
		};
	});
</script>

{#if renderedOpen && anchor && rootItems.length > 0}
	<div
		class={`gc-context-menu-layer ${closing ? 'is-closing' : ''} ${className}`}
		style={`--gc-context-menu-z:${zIndex};`}
		{@attach captureMenuLayerNode}
		{@attach portalMenuLayer}>
		{#each menuLayers as layer (`layer-${layer.depth}`)}
			{@const resolvedPosition = menuPositions[layer.depth]}
			{@const position = resolvedPosition ?? defaultMenuPosition(-10000, -10000)}
			<div
				class={`gc-context-menu ${menuClassName} ${layer.depth > 0 ? `gc-context-submenu ${submenuClassName}` : ''} ${resolvedPosition ? '' : 'is-positioning'}`}
				role="menu"
				tabindex="-1"
				style={menuStyle(layer, position)}
				{@attach captureMenuNode(layer.depth)}
				oncontextmenu={handleMenuContextMenu}
				transition:scale={{ duration: 130, start: 0.95, easing: cubicOut }}>
				{#each layer.items as item, index (`menu-item-${layer.depth}-${item.id ?? item.label ?? 'item'}-${index}`)}
					{@const submenuOpen = submenuPath[layer.depth] === index}
					{@const hasChildren = hasSubmenu(item)}
					{@const shortcutHint = itemShortcutHint(item)}
					{#if item.separator}
						<hr class="gc-context-separator" />
					{:else}
						<button
							type="button"
							role="menuitem"
							class={`gc-context-item ${itemClassName} ${item.className ?? ''} ${item.disabled ? 'is-disabled' : ''} ${submenuOpen ? 'submenu-open' : ''}`}
							style={`--gc-context-item-color:${item.color ?? 'inherit'};--gc-context-item-hover:${item.hoverColor ?? 'color-mix(in srgb, var(--gc-color-selection) 19%, transparent)'};`}
							disabled={item.disabled}
							{@attach captureItemNode(layer.depth, index)}
							onpointerenter={() => {
								handleItemPointerEnter(layer.depth, index, item);
							}}
							onclick={(event) => {
								handleItemClick(event, layer.depth, index, item);
							}}>
							{#if itemContent}
								{@render itemContent(item, layer.depth, submenuOpen)}
							{:else}
								<span class="gc-context-item-leading">
									{#if item.icon}
										<img class="gc-context-item-icon" src={item.icon} alt="" />
									{/if}
								</span>
								<span class="gc-context-item-label">{item.label}</span>
								{#if shortcutHint}
									<span class="gc-context-item-hint">{shortcutHint}</span>
								{/if}
								{#if hasChildren}
									<span class="gc-context-item-chevron" aria-hidden="true">></span>
								{/if}
							{/if}
						</button>
					{/if}
				{/each}
			</div>
		{/each}
	</div>
{/if}

<style>
	.gc-context-menu-layer {
		position: fixed;
		inset: 0;
		pointer-events: none;
		z-index: var(--gc-context-menu-z, 1300);
	}

	.gc-context-menu {
		--gc-context-menu-ease: cubic-bezier(0.22, 0.72, 0.2, 1);
		--gc-context-menu-duration: 0.18s;
		position: fixed;
		inline-size: max-content;
		min-inline-size: min(var(--gc-context-min-width, 10rem), var(--gc-context-available-width));
		max-inline-size: var(--gc-context-available-width, calc(100vw - 1rem));
		max-block-size: var(--gc-context-available-height, calc(100vh - 1rem));
		display: grid;
		grid-auto-flow: column;
		grid-template-rows: repeat(var(--gc-context-row-count, 1), minmax(0, max-content));
		grid-auto-columns: minmax(
			min(var(--gc-context-min-width, 10rem), var(--gc-context-available-width)),
			var(--gc-context-column-width, max-content)
		);
		gap: 0.05rem var(--gc-context-column-gap, 0.25rem);
		padding: 0.24rem;
		border-radius: 0.5rem;
		border: solid 0.0625rem rgba(from var(--gc-color-text) r g b / 5%);
		background:
			radial-gradient(
				circle at top right,
				rgba(from var(--gc-color-selection) r g b / 5%),
				transparent 58%
			),
			color-mix(in srgb, var(--gc-color-panel) 50%, rgba(8, 8, 8, 0.4));
		-webkit-backdrop-filter: blur(0.3rem);
		backdrop-filter: blur(0.3rem);
		box-shadow:
			0 0.75rem 2rem rgba(0, 0, 0, 0.5),
			0 0.12rem 0.42rem rgba(0, 0, 0, 0.3);
		pointer-events: auto;
		overflow: auto;
		overscroll-behavior: contain;
		scrollbar-width: thin;
		animation: gc-context-menu-enter var(--gc-context-menu-duration) var(--gc-context-menu-ease)
			both;
		transform-origin: var(--gc-context-origin-x, left) var(--gc-context-origin-y, top);
	}

	.gc-context-menu-layer.is-closing .gc-context-menu {
		animation: gc-context-menu-enter var(--gc-context-menu-duration) var(--gc-context-menu-ease)
			reverse both;
		pointer-events: none;
	}

	.gc-context-menu.is-positioning {
		opacity: 0;
		pointer-events: none;
		animation: none;
	}

	.gc-context-item {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr) auto auto;
		align-items: center;
		gap: 0.4rem;
		inline-size: 100%;
		min-inline-size: 0;
		min-block-size: 1.78rem;
		padding: 0.22rem 0.42rem;
		border: none;
		border-radius: 0.32rem;
		background: transparent;
		color: var(--gc-context-item-color, inherit);
		text-align: start;
		cursor: pointer;
		transition:
			background-color 0.14s ease,
			color 0.14s ease,
			opacity 0.14s ease;
	}

	.gc-context-item:hover,
	.gc-context-item:focus-visible,
	.gc-context-item.submenu-open {
		background: var(
			--gc-context-item-hover,
			color-mix(in srgb, var(--gc-color-selection) 19%, transparent)
		);
		outline: none;
	}

	.gc-context-item.is-disabled {
		opacity: 0.45;
		cursor: default;
	}

	.gc-context-item.is-disabled:hover {
		background: transparent;
	}

	.gc-context-item-leading {
		inline-size: 1rem;
		block-size: 1rem;
		display: inline-flex;
		align-items: center;
		justify-content: center;
	}

	.gc-context-item-icon {
		inline-size: 1rem;
		block-size: 1rem;
		display: block;
	}

	.gc-context-item-label {
		min-inline-size: 0;
		white-space: normal;
		overflow-wrap: anywhere;
		line-height: 1.18;
	}

	.gc-context-item-hint {
		justify-self: end;
		max-inline-size: min(16rem, 42vw);
		opacity: 0.68;
		font-size: 0.72em;
		text-align: end;
		white-space: normal;
		overflow-wrap: anywhere;
	}

	.gc-context-item-chevron {
		opacity: 0.68;
		font-size: 0.74em;
	}

	.gc-context-separator {
		margin: 0.12rem 0;
		inline-size: 100%;
		block-size: 0.0625rem;
		border: none;
		background: color-mix(in srgb, var(--gc-color-text) 16%, transparent);
	}

	@keyframes gc-context-menu-enter {
		from {
			opacity: 0;
			transform: translateY(0.22rem) scale(0.96);
		}
		to {
			opacity: 1;
			transform: translateY(0) scale(1);
		}
	}
</style>
