export interface ViewportContourState {
	selected?: boolean;
	hovered?: boolean;
	enabled?: boolean;
	valid?: boolean;
}

/** UI paint shared by DOM contours and a host's native overlay projection. Sizes are rem. */
export function viewportContourAppearance({
	selected = false,
	hovered = false,
	enabled = true,
	valid = true
}: ViewportContourState = {}) {
	return {
		fill: selected ? (valid ? '78aafa18' : 'a51d2c18') : '00000000',
		stroke: !valid ? 'ff5364' : selected ? '70aaff' : hovered ? 'c0d8ff' : 'a2b0c5',
		width: selected || (hovered && valid) ? 0.125 : 0.0625,
		dash: enabled ? null : ([0.3, 0.2] as const)
	};
}

interface ContourOptions extends Omit<ViewportContourState, 'hovered'> {
	identity?: string;
	onhover?(hovered: boolean): void;
}

/** Hover is presentation only; it never selects, captures input or starts a domain edit. */
export function viewportContour(node: SVGElement | HTMLElement, initial: ContourOptions = {}) {
	let options = initial;
	let hovered = false;
	const view = node.ownerDocument.defaultView!;
	const svg = node.namespaceURI === 'http://www.w3.org/2000/svg';
	function paint() {
		const appearance = viewportContourAppearance({ ...options, hovered });
		node.dataset.viewportHovered = String(hovered);
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
						: hovered
							? 'hover'
							: 'idle';
			node.style.borderColor = `var(--gc-viewport-contour-${state}, #${appearance.stroke})`;
			node.style.borderWidth = `${appearance.width}rem`;
			node.style.borderStyle = options.enabled === false ? 'dashed' : 'solid';
		}
	}
	function clear() {
		if (!hovered) return;
		hovered = false;
		view.removeEventListener('blur', clear);
		paint();
		options.onhover?.(false);
	}
	function enter(event: PointerEvent) {
		if (hovered || event.buttons || event.pointerType === 'touch') return;
		hovered = true;
		view.addEventListener('blur', clear);
		paint();
		options.onhover?.(true);
	}
	node.addEventListener('pointerenter', enter as EventListener);
	for (const event of ['pointerleave', 'pointerdown', 'pointercancel', 'lostpointercapture']) {
		node.addEventListener(event, clear);
	}
	paint();
	return {
		update(next: ContourOptions) {
			if (next.identity !== options.identity) clear();
			options = next;
			paint();
		},
		destroy() {
			clear();
			node.removeEventListener('pointerenter', enter as EventListener);
			for (const event of ['pointerleave', 'pointerdown', 'pointercancel', 'lostpointercapture']) {
				node.removeEventListener(event, clear);
			}
		}
	};
}
