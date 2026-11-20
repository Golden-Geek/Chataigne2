import { untrack } from 'svelte';

export interface ViewportContourState {
	/** Optional authored RGBA hex without '#'; selection/hover/errors retain their cues. */
	color?: string | null;
	selected?: boolean;
	hovered?: boolean;
	enabled?: boolean;
	valid?: boolean;
}

/** UI paint shared by DOM contours and a host's native overlay projection. Sizes are rem. */
export function viewportContourAppearance({
	color = null,
	selected = false,
	hovered = false,
	enabled = true,
	valid = true
}: ViewportContourState = {}) {
	return {
		fill: selected ? (valid ? '78aafa0f' : 'a51d2c18') : '00000000',
		stroke: !valid ? 'ff5364' : selected ? '70aaff' : hovered ? 'c0d8ff' : (color ?? 'a2b0c5'),
		width: selected || (hovered && valid) ? 0.1 : 0.0625,
		dash: enabled ? null : ([0.3, 0.2] as const)
	};
}

interface ContourOptions extends Omit<ViewportContourState, 'hovered'> {
	identity?: string;
	/** Optional enclosing hit area, resolved at mount. Focus and paint stay on the contour node. */
	hoverWithin?: string;
	onhover?(hovered: boolean): void;
	onfocus?(focused: boolean): void;
}

/** Hover is presentation only; it never selects, captures input or starts a domain edit. */
export function viewportContour(node: SVGElement | HTMLElement, initial: ContourOptions = {}) {
	let options = initial;
	let hovered = false;
	let focused = false;
	const view = node.ownerDocument.defaultView!;
	const svg = node.namespaceURI === 'http://www.w3.org/2000/svg';
	const pointerArea = (initial.hoverWithin && node.closest(initial.hoverWithin)) || node;
	node.dataset.viewportContour = '';
	function paint() {
		const appearance = viewportContourAppearance({ ...options, hovered: hovered || focused });
		node.dataset.viewportHovered = String(hovered);
		node.dataset.viewportFocused = String(focused);
		if (svg) {
			node.style.fill = `#${appearance.fill}`;
			node.style.stroke = `#${appearance.stroke}`;
			node.style.strokeWidth = `${appearance.width}rem`;
			node.style.strokeDasharray = appearance.dash?.map((part) => `${part}rem`).join(' ') ?? 'none';
			node.style.vectorEffect = 'non-scaling-stroke';
		} else {
			// DOM panels may theme contours; SVG/native paint keeps its shared color contract.
			const state =
				options.valid === false
					? 'invalid'
					: options.selected
						? 'selected'
						: hovered || focused
							? 'hover'
							: 'idle';
			node.style.borderColor =
				state === 'idle' && options.color
					? `#${appearance.stroke}`
					: `var(--gc-viewport-contour-${state}, #${appearance.stroke})`;
			node.style.borderWidth = `${appearance.width}rem`;
			node.style.borderStyle = options.enabled === false ? 'dashed' : 'solid';
		}
	}
	function clear() {
		if (!hovered) return;
		hovered = false;
		view.removeEventListener('blur', clear);
		paint();
		untrack(() => options.onhover?.(false));
	}
	function enter(event: PointerEvent) {
		if (hovered || event.buttons || event.pointerType === 'touch') return;
		hovered = true;
		view.addEventListener('blur', clear);
		paint();
		untrack(() => options.onhover?.(true));
	}
	function focus() {
		if (focused) return;
		focused = true;
		view.addEventListener('blur', blur);
		paint();
		untrack(() => options.onfocus?.(true));
	}
	function blur() {
		if (!focused) return;
		focused = false;
		view.removeEventListener('blur', blur);
		paint();
		// Moving or removing a focused contour can dispatch blur inside a template update.
		untrack(() => options.onfocus?.(false));
	}
	node.addEventListener('focus', focus);
	node.addEventListener('blur', blur);
	pointerArea.addEventListener('pointerenter', enter as EventListener);
	for (const event of ['pointerleave', 'pointerdown', 'pointercancel', 'lostpointercapture']) {
		pointerArea.addEventListener(event, clear);
	}
	paint();
	return {
		update(next: ContourOptions) {
			if (next.identity !== options.identity) {
				clear();
				blur();
			}
			options = next;
			paint();
		},
		destroy() {
			clear();
			blur();
			node.removeEventListener('focus', focus);
			node.removeEventListener('blur', blur);
			pointerArea.removeEventListener('pointerenter', enter as EventListener);
			for (const event of ['pointerleave', 'pointerdown', 'pointercancel', 'lostpointercapture']) {
				pointerArea.removeEventListener(event, clear);
			}
		}
	};
}
