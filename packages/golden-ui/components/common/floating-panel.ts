export interface ViewportRect {
	left: number;
	top: number;
	right: number;
	bottom: number;
	width: number;
	height: number;
}

export interface ViewportBounds extends ViewportRect {}

export const clampNumber = (value: number, min: number, max: number): number => {
	if (!Number.isFinite(value)) {
		return min;
	}
	if (max < min) {
		return min;
	}
	return Math.min(Math.max(value, min), max);
};

export const remToPx = (valueRem: number): number => {
	if (typeof window === 'undefined') {
		return valueRem * 16;
	}
	const rootFontSize = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
	const base = Number.isFinite(rootFontSize) ? rootFontSize : 16;
	return valueRem * base;
};

export const getMainViewportBounds = (): ViewportBounds => {
	if (typeof window === 'undefined') {
		return {
			left: 0,
			top: 0,
			right: 0,
			bottom: 0,
			width: 0,
			height: 0
		};
	}

	const appRect = document.querySelector('.gc-main')?.getBoundingClientRect() ?? null;
	const left = Math.max(0, appRect?.left ?? 0);
	const top = Math.max(0, appRect?.top ?? 0);
	const right = Math.min(window.innerWidth, appRect?.right ?? window.innerWidth);
	const bottom = Math.min(window.innerHeight, appRect?.bottom ?? window.innerHeight);

	return {
		left,
		top,
		right,
		bottom,
		width: Math.max(0, right - left),
		height: Math.max(0, bottom - top)
	};
};

interface AnchoredModalPlacementInput {
	bounds: ViewportBounds;
	panelWidth: number;
	anchorRect?: ViewportRect | null;
	marginXRem?: number;
	marginYRem?: number;
	minHeightRem?: number;
	preferredHeightRem?: number;
}

export interface AnchoredModalPlacement {
	left: number;
	top: number;
	height: number;
}

export const computeAnchoredModalPlacement = (
	input: AnchoredModalPlacementInput
): AnchoredModalPlacement => {
	const bounds = input.bounds;
	const marginXPx = remToPx(input.marginXRem ?? 0.75);
	const marginYPx = remToPx(input.marginYRem ?? 2);
	const minHeightPx = remToPx(input.minHeightRem ?? 15);
	const preferredHeightPx = remToPx(input.preferredHeightRem ?? 26);
	const maxPanelHeightPx = Math.max(minHeightPx, bounds.height - marginYPx * 2);

	let nextLeft = bounds.left + (bounds.width - input.panelWidth) / 2;
	let nextHeight = Math.min(preferredHeightPx, maxPanelHeightPx);
	let nextTop = bounds.top + (bounds.height - nextHeight) / 2;

	if (input.anchorRect) {
		const spaceBelow = bounds.bottom - input.anchorRect.bottom - marginYPx;
		const spaceAbove = input.anchorRect.top - bounds.top - marginYPx;
		const shouldOpenBelow = spaceBelow >= minHeightPx || spaceBelow >= spaceAbove;
		const preferredSpace = shouldOpenBelow ? spaceBelow : spaceAbove;
		nextHeight = Math.min(preferredHeightPx, Math.max(minHeightPx, preferredSpace));
		nextTop = shouldOpenBelow
			? input.anchorRect.bottom + marginYPx
			: input.anchorRect.top - nextHeight - marginYPx;
		nextLeft = input.anchorRect.left;
	}

	const minLeft = bounds.left + marginXPx;
	const maxLeft = Math.max(minLeft, bounds.right - input.panelWidth - marginXPx);
	const minTop = bounds.top + marginYPx;
	const maxTop = Math.max(minTop, bounds.bottom - nextHeight - marginYPx);
	const left = clampNumber(nextLeft, minLeft, maxLeft);
	const top = clampNumber(nextTop, minTop, maxTop);
	const height = Math.min(nextHeight, Math.max(minHeightPx, bounds.bottom - top - marginYPx));

	return {
		left,
		top,
		height
	};
};

interface PointAnchoredMenuPlacementInput {
	bounds: ViewportBounds;
	anchorX: number;
	anchorY: number;
	menuWidth: number;
	menuHeight: number;
	marginRem?: number;
}

export interface PointAnchoredMenuPlacement {
	left: number;
	top: number;
	originX: 'left' | 'right';
	originY: 'top' | 'bottom';
}

export const computePointAnchoredMenuPlacement = (
	input: PointAnchoredMenuPlacementInput
): PointAnchoredMenuPlacement => {
	const bounds = input.bounds;
	const marginPx = remToPx(input.marginRem ?? 0.45);
	const minLeft = bounds.left + marginPx;
	const maxLeft = Math.max(minLeft, bounds.right - input.menuWidth - marginPx);
	const minTop = bounds.top + marginPx;
	const maxTop = Math.max(minTop, bounds.bottom - input.menuHeight - marginPx);

	const left = clampNumber(input.anchorX, minLeft, maxLeft);
	const top = clampNumber(input.anchorY, minTop, maxTop);

	return {
		left,
		top,
		originX: left + 0.5 < input.anchorX ? 'right' : 'left',
		originY: top + 0.5 < input.anchorY ? 'bottom' : 'top'
	};
};
