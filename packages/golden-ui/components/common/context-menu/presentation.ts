import { clampNumber, getMainViewportBounds, remToPx } from '../floating-panel';
import { LARGE_MENU_THRESHOLD } from '../context-menu';

export interface MenuPosition {
	left: number;
	top: number;
	originX: 'left' | 'right';
	originY: 'top' | 'bottom';
}

export function menuPresentationStyle(
	itemCount: number,
	position: MenuPosition,
	minWidthRem: number,
	maxWidthCss: string,
	viewportPaddingRem: number
): string {
	const bounds = getMainViewportBounds();
	const margin = remToPx(viewportPaddingRem);
	const availableWidth = Math.max(1, bounds.width - margin * 2);
	const availableHeight = Math.max(1, bounds.height - margin * 2);
	const gap = remToPx(0.25);
	const columns = Math.max(
		1,
		Math.floor((availableWidth + gap) / (Math.min(availableWidth, remToPx(minWidthRem)) + gap))
	);
	const count = Math.max(1, itemCount);
	// Large menus have one windowed child, rather than one CSS grid track per logical row.
	const rowCount =
		itemCount > LARGE_MENU_THRESHOLD
			? 1
			: clampNumber(
					Math.max(
						Math.min(count, Math.floor(availableHeight / remToPx(1.95))),
						Math.ceil(count / columns)
					),
					1,
					count
				);
	const wraps = itemCount <= LARGE_MENU_THRESHOLD && rowCount < itemCount;
	const columnWidth =
		maxWidthCss.trim() === 'max-content'
			? wraps
				? `${Math.min(availableWidth, Math.max(remToPx(minWidthRem), remToPx(22)))}px`
				: 'max-content'
			: maxWidthCss;
	return [
		`left:${position.left}px`,
		`top:${position.top}px`,
		`--gc-context-min-width:${minWidthRem}rem`,
		`--gc-context-max-width:${maxWidthCss}`,
		`--gc-context-column-width:${columnWidth}`,
		`--gc-context-collection-width:${maxWidthCss.trim() === 'max-content' ? '22rem' : maxWidthCss}`,
		`--gc-context-available-width:${availableWidth}px`,
		`--gc-context-available-height:${availableHeight}px`,
		`--gc-context-row-count:${rowCount}`,
		`--gc-context-column-gap:0.25rem`,
		`--gc-context-origin-x:${position.originX}`,
		`--gc-context-origin-y:${position.originY}`
	].join(';');
}
