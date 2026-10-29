import type { CssRect } from './generated/CssRect';

export interface ClipAncestor {
	rect: CssRect;
	clipX: boolean;
	clipY: boolean;
}

export function intersectRect(a: CssRect, b: CssRect): CssRect {
	const x = Math.max(a.x, b.x);
	const y = Math.max(a.y, b.y);
	return {
		x,
		y,
		width: Math.max(0, Math.min(a.x + a.width, b.x + b.width) - x),
		height: Math.max(0, Math.min(a.y + a.height, b.y + b.height) - y)
	};
}

/** CSS layout coordinates only; DPI/zoom conversion belongs to the authentic native host. */
export function clipPreviewRect(
	rect: CssRect,
	viewport: CssRect,
	ancestors: readonly ClipAncestor[]
): CssRect {
	let clip = intersectRect(rect, viewport);
	for (const ancestor of ancestors) {
		const bounds = {
			x: ancestor.clipX ? ancestor.rect.x : clip.x,
			y: ancestor.clipY ? ancestor.rect.y : clip.y,
			width: ancestor.clipX ? ancestor.rect.width : clip.width,
			height: ancestor.clipY ? ancestor.rect.height : clip.height
		};
		clip = intersectRect(clip, bounds);
	}
	return clip;
}

const fromRect = (rect: DOMRect): CssRect => ({
	x: rect.x,
	y: rect.y,
	width: rect.width,
	height: rect.height
});
const clips = (value: string): boolean =>
	['auto', 'scroll', 'hidden', 'clip', 'overlay'].includes(value);

function clientRect(element: HTMLElement): CssRect {
	const bounds = element.getBoundingClientRect();
	const scaleX = element.offsetWidth ? bounds.width / element.offsetWidth : 1;
	const scaleY = element.offsetHeight ? bounds.height / element.offsetHeight : 1;
	return {
		x: bounds.x + element.clientLeft * scaleX,
		y: bounds.y + element.clientTop * scaleY,
		width: element.clientWidth * scaleX,
		height: element.clientHeight * scaleY
	};
}

export function measurePreview(element: HTMLElement, requestedVisible: boolean) {
	const rect = fromRect(element.getBoundingClientRect());
	const visual = window.visualViewport;
	const viewport: CssRect = visual
		? { x: visual.offsetLeft, y: visual.offsetTop, width: visual.width, height: visual.height }
		: { x: 0, y: 0, width: window.innerWidth, height: window.innerHeight };
	const ancestors: ClipAncestor[] = [];
	let visible = requestedVisible && element.isConnected && !document.hidden;
	for (let ancestor: HTMLElement | null = element; ancestor; ancestor = ancestor.parentElement) {
		const style = getComputedStyle(ancestor);
		visible &&=
			style.display !== 'none' &&
			style.visibility !== 'hidden' &&
			style.visibility !== 'collapse' &&
			Number(style.opacity) === 1;
		// Rectangular native presentation cannot follow rotated/skewed DOM clipping.
		if (style.transform !== 'none') {
			const transform = new DOMMatrixReadOnly(style.transform);
			visible &&= transform.is2D && transform.b === 0 && transform.c === 0;
		}
		if (ancestor !== element)
			ancestors.push({
				rect: clientRect(ancestor),
				clipX: clips(style.overflowX),
				clipY: clips(style.overflowY)
			});
	}
	const clip = clipPreviewRect(rect, viewport, ancestors);
	visible &&= rect.width > 0 && rect.height > 0 && clip.width > 0 && clip.height > 0;
	return { rect, clip, visible };
}
