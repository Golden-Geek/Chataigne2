import type { CommandId } from '../../store/commands.svelte';

export type ContextMenuPlacement =
	| 'bottom-start'
	| 'bottom-end'
	| 'top-start'
	| 'top-end'
	| 'right-start'
	| 'right-end'
	| 'left-start'
	| 'left-end';

export interface ContextMenuPointAnchor {
	kind: 'point';
	x: number;
	y: number;
}

export interface ContextMenuElementAnchor {
	kind: 'element';
	element: HTMLElement | null;
	placement?: ContextMenuPlacement;
	offsetRem?: number;
}

export type ContextMenuAnchor = ContextMenuPointAnchor | ContextMenuElementAnchor;

export interface ContextMenuItem {
	id?: string;
	label?: string;
	icon?: string;
	hint?: string;
	commandId?: CommandId;
	separator?: boolean;
	visible?: boolean;
	disabled?: boolean;
	color?: string;
	hoverColor?: string;
	className?: string;
	closeOnSelect?: boolean;
	submenu?: readonly ContextMenuItem[];
	action?: (event: MouseEvent, item: ContextMenuItem) => void | Promise<void>;
}

export const normalizeContextMenuItems = (
	items: readonly ContextMenuItem[] | null | undefined
): ContextMenuItem[] => {
	if (!items || items.length === 0) {
		return [];
	}

	const normalized: ContextMenuItem[] = [];
	let previousWasSeparator = true;

	for (const item of items) {
		if (item.visible === false) {
			continue;
		}

		if (item.separator) {
			if (previousWasSeparator) {
				continue;
			}
			normalized.push(item);
			previousWasSeparator = true;
			continue;
		}

		if (!item.label || item.label.trim().length === 0) {
			continue;
		}

		normalized.push(item);
		previousWasSeparator = false;
	}

	while (normalized.length > 0 && normalized[normalized.length - 1]?.separator) {
		normalized.pop();
	}

	return normalized;
};
