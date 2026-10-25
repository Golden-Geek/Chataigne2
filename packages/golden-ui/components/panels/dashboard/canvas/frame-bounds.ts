import type { FreeLayoutRect } from './contracts';

export function clipFrameBounds(
	rect: FreeLayoutRect,
	clip: FreeLayoutRect,
	x: boolean,
	y: boolean
): FreeLayoutRect {
	// DOMRect exposes coordinates through prototype getters, not enumerable own fields.
	const result = { left: rect.left, top: rect.top, width: rect.width, height: rect.height };
	if (x) {
		const left = Math.max(rect.left, clip.left),
			right = Math.min(rect.left + rect.width, clip.left + clip.width);
		result.left = right > left ? left : clip.left;
		result.width = right > left ? right - left : clip.width;
	}
	if (y) {
		const top = Math.max(rect.top, clip.top),
			bottom = Math.min(rect.top + rect.height, clip.top + clip.height);
		result.top = bottom > top ? top : clip.top;
		result.height = bottom > top ? bottom - top : clip.height;
	}
	return result;
}

export function surfaceViewport(element: HTMLElement): FreeLayoutRect {
	const box = element.getBoundingClientRect();
	const x = box.width / Math.max(1, element.offsetWidth),
		y = box.height / Math.max(1, element.offsetHeight);
	return {
		left: box.left + element.clientLeft * x,
		top: box.top + element.clientTop * y,
		width: element.clientWidth * x,
		height: element.clientHeight * y
	};
}

/** The camera can move its frame, but cannot expose content through an inner clip. */
export function framePresentationBounds(
	rect: FreeLayoutRect,
	anchor: HTMLElement,
	frame: HTMLElement
): FreeLayoutRect {
	let result = rect;
	for (
		let element = anchor.parentElement;
		element && element !== frame;
		element = element.parentElement
	) {
		const style = getComputedStyle(element);
		result = clipFrameBounds(
			result,
			surfaceViewport(element),
			style.overflowX !== 'visible',
			style.overflowY !== 'visible'
		);
	}
	return result;
}

/** Reveal locally without scrolling the app document or changing the page camera. */
export function revealInSurface(surface: HTMLElement, target: HTMLElement): void {
	const style = getComputedStyle(surface),
		clip = surfaceViewport(surface),
		rect = target.getBoundingClientRect();
	const box = surface.getBoundingClientRect();
	if (
		['auto', 'scroll'].includes(style.overflowX) &&
		(rect.left < clip.left || rect.right > clip.left + clip.width)
	)
		surface.scrollLeft +=
			(rect.left + rect.right - 2 * clip.left - clip.width) /
			2 /
			(box.width / Math.max(1, surface.offsetWidth));
	if (
		['auto', 'scroll'].includes(style.overflowY) &&
		(rect.top < clip.top || rect.bottom > clip.top + clip.height)
	)
		surface.scrollTop +=
			(rect.top + rect.bottom - 2 * clip.top - clip.height) /
			2 /
			(box.height / Math.max(1, surface.offsetHeight));
}
