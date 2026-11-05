import type { CssRect } from './generated/CssRect';
import { MAX_RETIREMENT_COVERS } from './retirement';

const namespace = 'http://www.w3.org/2000/svg';
const svg = <K extends keyof SVGElementTagNameMap>(tag: K) =>
	document.createElementNS(namespace, tag);

/** One union of historical coverage minus every retained live aperture. SVG compositing
 * avoids both cross-view clipping and rectangle-fragment growth when apertures overlap. */
export class RetirementLayer {
	readonly element = svg('svg');
	private readonly coverage = svg('g');
	private readonly apertures = svg('g');
	private readonly rectangles = new Map<string, SVGRectElement>();

	constructor() {
		const id = `native-retirement-${crypto.randomUUID()}`;
		this.element.dataset.nativePreviewRetirementLayer = '';
		this.element.setAttribute('aria-hidden', 'true');
		Object.assign(this.element.style, {
			position: 'fixed',
			inset: '0',
			width: '100%',
			height: '100%',
			pointerEvents: 'none',
			zIndex: '-1'
		});
		const definitions = svg('defs');
		const mask = svg('mask');
		mask.id = id;
		mask.setAttribute('maskUnits', 'userSpaceOnUse');
		mask.setAttribute('x', '0');
		mask.setAttribute('y', '0');
		mask.setAttribute('width', '100%');
		mask.setAttribute('height', '100%');
		mask.style.maskType = 'luminance';
		this.coverage.setAttribute('fill', 'white');
		this.apertures.setAttribute('fill', 'black');
		mask.append(this.coverage, this.apertures);
		definitions.append(mask);
		const paint = svg('rect');
		paint.setAttribute('width', '100%');
		paint.setAttribute('height', '100%');
		paint.style.fill = 'var(--gc-color-background, #000)';
		paint.setAttribute('mask', `url(#${id})`);
		this.element.append(definitions, paint);
	}

	update(
		covers: ReadonlyArray<readonly [string, CssRect]>,
		apertures: ReadonlyArray<readonly [string, CssRect]>
	) {
		if (covers.length > MAX_RETIREMENT_COVERS || apertures.length > MAX_RETIREMENT_COVERS)
			throw new Error('Native retirement coverage capacity exceeded');
		const current = new Set<string>();
		for (const [kind, entries, parent] of [
			['cover', covers, this.coverage],
			['aperture', apertures, this.apertures]
		] as const) {
			for (const [id, rect] of entries) {
				const key = `${kind}:${id}`;
				current.add(key);
				let element = this.rectangles.get(key);
				if (!element) {
					element = svg('rect');
					element.dataset.nativePreviewRetirement = key;
					parent.append(element);
					this.rectangles.set(key, element);
				}
				for (const [name, value] of Object.entries(rect)) {
					const text = String(value);
					if (element.getAttribute(name) !== text) element.setAttribute(name, text);
				}
			}
		}
		for (const [id, element] of this.rectangles) {
			if (!current.has(id)) {
				element.remove();
				this.rectangles.delete(id);
			}
		}
	}
}
